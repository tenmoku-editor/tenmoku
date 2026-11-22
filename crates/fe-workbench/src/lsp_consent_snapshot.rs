//! Immutable, content-addressed snapshots of the extension packages whose
//! language servers the user granted (extensions record §1, Q5).
//!
//! A granted server runs from `<consent file's dir>/extensions-lsp-packages/
//! <package hash>/`, never from the extension's live, mutable directory, so
//! an update or an edit can't change what a running server loads (a late
//! `require`, a child process) or what a restart runs before the user has
//! accepted the change. Without a consent file the store is a private
//! per-session temporary directory, made fresh and owned by the session.
//!
//! - **Made** off the main thread: the canonical directory is copied (never
//!   hard linked: an in-place rewrite of an installed file would change the
//!   snapshot) into a `.staging-*` directory, hashed from the copy, made
//!   read-only, and only then renamed to its hash, so a directory with a
//!   hash's name is finished once it is read-only. Its own write bits are
//!   cleared last, after the rename, as macOS 15 doesn't rename a directory
//!   its owner can't write; until then a check finds it unfinished, as it
//!   would one a crash left there. A symlink is copied as one, unfollowed;
//!   one reaching back into the live directory makes the package not
//!   snapshottable, checked on every evaluation, reuse included. A reused
//!   snapshot is hashed again and checked read-only; a damaged one is
//!   replaced if nothing holds it, refused otherwise.
//! - **Kept** while a grant in the consent file names it, or a lock is held
//!   on its `<hash>.lock`: this app holds a shared one for each hash its
//!   servers currently have or have pending, and each launched process one
//!   until it is gone, so another app instance sharing the store never
//!   collects it. A lock file is never removed, so its identity is stable
//!   for everything waiting on it. Collected, off the main thread, only
//!   after taking that lock exclusively without blocking.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use kiln_l10n::verbatim;

use crate::lsp_consent::{ConsentStore, Decision, on_path, package_hash_or_why};

/// The store's directory beside the consent file.
const STORE_DIR: &str = "extensions-lsp-packages";
const STAGING: &str = ".staging-";
/// A lock file's suffix.
const LOCK: &str = verbatim!(".lock");

/// The steps of making a snapshot, as a failure names them: checking the
/// live package's links, hashing it, reusing a snapshot found, taking a
/// lock, making the staging directory, copying into it, publishing the
/// copy (hashed and made read-only) and renaming it to its hash.
const CHECK: &str = "check";
const HASH: &str = "hash";
const REUSE: &str = "reuse";
const LOCKING: &str = "lock";
const STAGE: &str = "stage";
const COPY: &str = "copy";
const PUBLISH: &str = "publish";
const RENAME: &str = "rename";

/// `error`, met at `step` on `path`, as an error naming both; its kind kept.
fn failed(step: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{step} {}: {error}", path.display()))
}

/// [`failed`], for `map_err`.
fn at<'a>(step: &'a str, path: &'a Path) -> impl FnOnce(io::Error) -> io::Error + 'a {
    move |error| failed(step, path, error)
}

/// The snapshots of this app: shared by the evaluations that make them and
/// every launch and process that holds one.
#[derive(Clone, Default)]
pub(crate) struct Snapshots(Arc<Shared>);

#[derive(Default)]
struct Shared {
    /// Held for each making and each collection, so neither sees the other
    /// half done.
    state: Mutex<State>,
    /// The store's directory as the last evaluation set it, readable while
    /// one is under way.
    dir: Mutex<Option<PathBuf>>,
}

#[derive(Default)]
struct State {
    consent_file: Option<PathBuf>,
    /// Hashes this session keeps: its servers' current and pending
    /// invocations, and its own decisions.
    refs: HashSet<String>,
    /// A shared lock on the snapshot of each hash this session's servers
    /// currently have or have pending, for as long as it does.
    held: HashMap<String, fs::File>,
    /// The private store of a session without a consent file.
    temporary: Option<PathBuf>,
}

impl State {
    /// The store's directory; an error if a session without a consent file
    /// can't have its private one.
    fn dir(&mut self) -> io::Result<PathBuf> {
        if let Some(file) = &self.consent_file {
            return Ok(file
                .parent()
                .filter(|dir| !dir.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .join(STORE_DIR));
        }
        if self.temporary.is_none() {
            self.temporary = Some(private_temporary_dir()?);
        }
        Ok(self.temporary.clone().unwrap_or_default())
    }
}

/// A new directory only the user can use, in the temporary directory: one
/// that already exists, by any name, is never accepted, and nothing is
/// returned unless this call made it.
fn private_temporary_dir() -> io::Result<PathBuf> {
    private_dir_in(&std::env::temp_dir())
}

/// [`private_temporary_dir`] in `base`, for tests to reach a failure.
pub(crate) fn private_dir_in(base: &Path) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..100 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos());
        let path = base.join(format!(
            "{}-{}-{}-{nanos:x}",
            verbatim!("fastedit-lsp-packages"),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(unix)]
        let mut builder = fs::DirBuilder::new();
        #[cfg(not(unix))]
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(verbatim!(
        "no unused name for a private snapshot directory"
    )))
}

impl Drop for Shared {
    fn drop(&mut self) {
        let state = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
        state.held.clear();
        if let Some(dir) = state.temporary.take() {
            make_writable(&dir);
            let _ = fs::remove_dir_all(dir);
        }
    }
}

/// A shared lock on a snapshot, released when dropped.
pub(crate) struct Hold {
    _lock: fs::File,
}

/// A snapshot a launch runs from.
pub(crate) struct Held {
    pub(crate) hold: Hold,
    pub(crate) dir: PathBuf,
}

impl Snapshots {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn dir(&self) -> Option<PathBuf> {
        self.0
            .dir
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Starts work on the store of `consent_file` (or the private one):
    /// nothing else of this app touches it until the guard drops.
    pub(crate) fn begin(&self, consent_file: Option<&Path>) -> io::Result<Session<'_>> {
        let mut state = self.state();
        state.consent_file = consent_file.map(Path::to_owned);
        let dir = state.dir();
        *self.0.dir.lock().unwrap_or_else(PoisonError::into_inner) = dir.as_ref().ok().cloned();
        Ok(Session {
            state,
            dir: dir?,
            fresh: HashMap::new(),
        })
    }

    /// Takes a shared lock on the snapshot of `hash` and checks it, hashed
    /// again, is exactly that package. `None` if it's missing or isn't.
    /// Blocking: reads the package.
    pub(crate) fn hold(&self, hash: &str) -> Option<Held> {
        let dir = self.dir()?;
        if !is_hash(hash) {
            return None;
        }
        let lock = lock_file(&dir, hash).ok()?;
        lock.lock_shared().ok()?;
        let canonical = dunce::canonicalize(dir.join(hash)).ok()?;
        sound(&canonical, hash).then_some(Held {
            hold: Hold { _lock: lock },
            dir: canonical,
        })
    }

    /// Collects what's no longer kept, on a thread of its own.
    pub(crate) fn collect_in_background(&self) {
        let this = self.clone();
        std::thread::spawn(move || {
            let mut state = this.state();
            if let Ok(dir) = state.dir() {
                collect(&dir, &state);
            }
        });
    }
}

/// The store while work on it is under way.
pub(crate) struct Session<'a> {
    state: MutexGuard<'a, State>,
    dir: PathBuf,
    /// The locks this session took on snapshots it made or found, until
    /// [`Session::keep`] settles which stay.
    fresh: HashMap<String, fs::File>,
}

impl Session<'_> {
    /// The canonical directory of the finished snapshot of `hash` if one is
    /// there and its content is that package, with a shared lock on it
    /// kept until the hash isn't [kept](Session::keep). Blocking.
    pub(crate) fn verified(&mut self, hash: &str) -> Option<PathBuf> {
        if !is_hash(hash) {
            return None;
        }
        let lock = lock_file(&self.dir, hash).ok()?;
        lock.lock_shared().ok()?;
        let dir = dunce::canonicalize(self.dir.join(hash)).ok()?;
        sound(&dir, hash).then(|| {
            self.fresh.insert(hash.to_owned(), lock);
            dir
        })
    }

    /// A snapshot of the extension in the canonical directory `live`: its
    /// hash, and where it is, with a shared lock on it kept until the hash
    /// isn't [kept](Session::keep). Blocking: reads, and may copy, the
    /// package.
    pub(crate) fn snapshot(&mut self, live: &Path) -> io::Result<(String, PathBuf)> {
        // Whether it can be snapshotted at all depends on where it is, so
        // it's checked for a snapshot found as much as one made.
        check_live_links(live, live, Path::new(""))?;
        let hash = package_hash_or_why(live).map_err(at(HASH, live))?;
        if let Some(found) = self.reuse(&hash)? {
            return Ok((hash, found));
        }
        ensure_dir(&self.dir).map_err(at(STAGE, &self.dir))?;
        let (name, lock) = staging_slot(&self.dir)?;
        let staging = self.dir.join(&name);
        // Left by an earlier run of this process id: nothing holds it.
        make_writable(&staging);
        let _ = fs::remove_dir_all(&staging);
        let result = self
            .stage(live, &staging)
            .and_then(|()| self.publish(&staging));
        if result.is_err() {
            make_writable(&staging);
            let _ = fs::remove_dir_all(&staging);
        }
        drop(lock);
        result
    }

    /// The snapshot of `hash` if one is there: locked, hashed again and
    /// checked finished. One damaged is removed if nothing else holds it,
    /// and an error if something does.
    fn reuse(&mut self, hash: &str) -> io::Result<Option<PathBuf>> {
        let target = self.dir.join(hash);
        if !target.is_dir() {
            return Ok(None);
        }
        let lock = shared_lock(&self.dir, hash)?;
        let found = dunce::canonicalize(&target).map_err(at(REUSE, &target));
        let damage = match found.and_then(|found| soundness(&found, hash).map(|()| found)) {
            Ok(found) => {
                self.fresh.insert(hash.to_owned(), lock);
                return Ok(Some(found));
            }
            Err(damage) => damage,
        };
        // Damaged, or gone meanwhile.
        lock.unlock()
            .map_err(at(LOCKING, &lock_path(&self.dir, hash)))?;
        if lock.try_lock().is_err() {
            return Err(failed(
                REUSE,
                &target,
                io::Error::other(format!(
                    "{}{damage}",
                    verbatim!("the snapshot is damaged and in use: ")
                )),
            ));
        }
        if target.is_dir() {
            make_writable(&target);
            fs::remove_dir_all(&target).map_err(at(REUSE, &target))?;
        }
        Ok(None)
    }

    /// Copies `live` to the new `staging`, refusing a symlink that reaches
    /// back into it.
    fn stage(&self, live: &Path, staging: &Path) -> io::Result<()> {
        fs::create_dir(staging).map_err(at(STAGE, staging))?;
        let staged = dunce::canonicalize(staging).map_err(at(STAGE, staging))?;
        copy_tree(live, live, staging, Path::new(""))?;
        // A relative link through a link of its own may still leave.
        check_links(live, &staged, &staged)
    }

    /// Makes what's in the staged copy read-only and renames it to the hash
    /// of its own content, under a shared lock on that hash, kept, and then
    /// makes the directory itself read-only; if one is there already, that
    /// one is used if it is sound.
    fn publish(&mut self, staging: &Path) -> io::Result<(String, PathBuf)> {
        let staged = dunce::canonicalize(staging).map_err(at(PUBLISH, staging))?;
        let hash = package_hash_or_why(&staged).map_err(at(PUBLISH, &staged))?;
        // Not the directory itself yet: macOS 15 refuses to rename a
        // directory its owner can't write (EACCES, on the GitHub runner's
        // APFS; T2083), where macOS 27 and Linux, renaming within one
        // directory, don't.
        make_readonly_within(staging)?;
        // Taken before the rename, so a collection removing this hash
        // finishes first and none starts after.
        let lock = shared_lock(&self.dir, &hash)?;
        let target = self.dir.join(&hash);
        if let Err(error) = fs::rename(staging, &target) {
            // Another instance made it first (or something is in the way).
            let renamed = || {
                io::Error::new(
                    error.kind(),
                    format!(
                        "{RENAME} {} -> {}: {error}",
                        staging.display(),
                        target.display()
                    ),
                )
            };
            let found = dunce::canonicalize(&target).map_err(|_| renamed())?;
            soundness(&found, &hash).map_err(|damage| {
                io::Error::new(
                    damage.kind(),
                    format!("{}{}{damage}", renamed(), verbatim!("; the one there: ")),
                )
            })?;
            make_writable(staging);
            fs::remove_dir_all(staging).map_err(at(PUBLISH, staging))?;
            self.fresh.insert(hash.clone(), lock);
            return Ok((hash, found));
        }
        // Until this, a check of it finds it unfinished, as one left by a
        // crash here would be: replaced if nothing holds it.
        set_readonly(&target)?;
        let found = dunce::canonicalize(&target).map_err(at(RENAME, &target))?;
        self.fresh.insert(hash.clone(), lock);
        Ok((hash, found))
    }

    /// Keeps `hashes` through every collection of this session, and the
    /// locks on them; the locks on the rest are let go.
    pub(crate) fn keep(&mut self, hashes: HashSet<String>) {
        let fresh = std::mem::take(&mut self.fresh);
        let held = &mut self.state.held;
        held.retain(|hash, _| hashes.contains(hash));
        for (hash, lock) in fresh {
            if hashes.contains(&hash) {
                held.insert(hash, lock);
            }
        }
        self.state.refs = hashes;
    }

    /// Collects what's no longer kept.
    pub(crate) fn collect(&self) {
        collect(&self.dir, &self.state);
    }
}

/// Whether the directory `dir` is the finished snapshot of `hash`: that
/// content, and nothing in it writable.
fn sound(dir: &Path, hash: &str) -> bool {
    soundness(dir, hash).is_ok()
}

/// [`sound`], or why not: what its content hashes to, or what in it is
/// writable or can't be read.
fn soundness(dir: &Path, hash: &str) -> io::Result<()> {
    let found = package_hash_or_why(dir)?;
    if found != hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}{found}", verbatim!("its content hashes to ")),
        ));
    }
    read_only(dir)
}

/// Nothing at or under `path` (never through a symlink) is writable; an
/// error naming the first that is, or that can't be checked.
fn read_only(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|error| on_path(path, error))?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if !meta.permissions().readonly() {
        return Err(on_path(
            path,
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}{:o}",
                    verbatim!("writable, mode "),
                    permissions_mode(&meta)
                ),
            ),
        ));
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| on_path(path, error))? {
            read_only(&entry.map_err(|error| on_path(path, error))?.path())?;
        }
    }
    Ok(())
}

/// The mode bits, for a message; 0 off Unix.
fn permissions_mode(meta: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.permissions().mode()
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        0
    }
}

/// Removes the snapshots and staging directories in `dir` that nothing
/// keeps and no lock holds.
fn collect(dir: &Path, state: &State) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names {
        if name.ends_with(LOCK) {
            continue;
        }
        if name.starts_with(STAGING) {
            collect_staging(dir, &name);
        } else if is_hash(&name) && !state.refs.contains(&name) {
            collect_snapshot(dir, &name, state);
        }
    }
}

/// An abandoned staging directory that nothing holds. Its lock file stays
/// (the names are reused, so it is never unlinked from under an opener).
fn collect_staging(dir: &Path, name: &str) {
    let Ok(lock) = lock_file(dir, name) else {
        return;
    };
    if lock.try_lock().is_err() {
        return;
    }
    let path = dir.join(name);
    make_writable(&path);
    let _ = fs::remove_dir_all(&path);
}

/// The snapshot of `hash`, if nothing holds it and no grant names it. Its
/// lock file stays, as others may be waiting on it.
fn collect_snapshot(dir: &Path, hash: &str, state: &State) {
    let Ok(lock) = lock_file(dir, hash) else {
        return;
    };
    // Held by a process running from it, or an app keeping it: left be.
    if lock.try_lock().is_err() {
        return;
    }
    // Read with the lock held, so a grant is either seen or can't be
    // launched from it.
    if granted_in_file(state).is_none_or(|granted| granted.contains(hash)) {
        return;
    }
    let path = dir.join(hash);
    make_writable(&path);
    if let Err(error) = fs::remove_dir_all(&path) {
        log::debug!("can't remove {}: {error}", path.display());
    }
}

/// The package hashes the consent file grants; `None` if it can't be read,
/// so nothing is removed on a guess.
fn granted_in_file(state: &State) -> Option<HashSet<String>> {
    let Some(file) = &state.consent_file else {
        return Some(HashSet::new());
    };
    let entries = ConsentStore::new(file).read().ok()?;
    Some(
        entries
            .into_values()
            .filter(|entry| entry.decision == Decision::Granted)
            .map(|entry| entry.invocation.package_hash)
            .collect(),
    )
}

/// Whether `name` is a package hash as
/// [`package_hash`](crate::lsp_consent::package_hash) writes it.
fn is_hash(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A staging name of this process's, free and locked: the lowest slot
/// whose lock nothing holds. The lock files are reused and never removed.
pub(crate) fn staging_slot(dir: &Path) -> io::Result<(String, fs::File)> {
    for slot in 0..1024 {
        let name = format!("{STAGING}{}-{slot}", std::process::id());
        let lock = lock_file(dir, &name)?;
        #[cfg(test)]
        after_open(dir);
        if lock.try_lock().is_ok() {
            return Ok((name, lock));
        }
    }
    Err(failed(
        STAGE,
        dir,
        io::Error::other(verbatim!("no free staging directory")),
    ))
}

/// `dir`, made, and only the user's if new.
fn ensure_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

/// `<name>.lock` in `dir`.
fn lock_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{LOCK}"))
}

/// `<name>.lock` in `dir`, made if missing; an error names the step and
/// the path.
fn lock_file(dir: &Path, name: &str) -> io::Result<fs::File> {
    ensure_dir(dir).map_err(at(LOCKING, dir))?;
    let path = lock_path(dir, name);
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(at(LOCKING, &path))
}

/// [`lock_file`], with a shared lock taken on it.
fn shared_lock(dir: &Path, name: &str) -> io::Result<fs::File> {
    let lock = lock_file(dir, name)?;
    lock.lock_shared()
        .map_err(at(LOCKING, &lock_path(dir, name)))?;
    Ok(lock)
}

/// Refuses a symlink anywhere under `dir` (the canonical live directory
/// `root`, `rel` the way to it) that reaches back into the live directory.
fn check_live_links(root: &Path, dir: &Path, rel: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir).map_err(at(CHECK, dir))? {
        let entry = entry.map_err(at(CHECK, dir))?;
        let path = entry.path();
        let rel = rel.join(entry.file_name());
        let kind = entry.file_type().map_err(at(CHECK, &path))?;
        if kind.is_symlink() {
            let target = fs::read_link(&path).map_err(at(CHECK, &path))?;
            refuse_link(root, &rel, &target).map_err(at(CHECK, &path))?;
            if link_escapes(root, root, dir, &target) {
                return Err(failed(
                    CHECK,
                    &path,
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "{}{}",
                            rel.display(),
                            verbatim!(" reaches outside the package or the live directory")
                        ),
                    ),
                ));
            }
        } else if kind.is_dir() {
            check_live_links(root, &entry.path(), &rel)?;
        }
    }
    Ok(())
}

fn refuse_link(root: &Path, rel: &Path, target: &Path) -> io::Result<()> {
    if reaches_out(root, rel, target) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}{}{}{}",
                rel.display(),
                verbatim!(" is a symlink to "),
                target.display(),
                verbatim!(", which reaches the live extension directory")
            ),
        ));
    }
    Ok(())
}

/// Copies `src`, under `root` (the live directory), to `dst`: files by
/// `fs::copy`, symlinks as symlinks, with the target unchanged.
fn copy_tree(root: &Path, src: &Path, dst: &Path, rel: &Path) -> io::Result<()> {
    let mut entries = fs::read_dir(src)
        .and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
        .map_err(at(COPY, src))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let from = entry.path();
        let rel = rel.join(entry.file_name());
        let to = dst.join(entry.file_name());
        let kind = entry.file_type().map_err(at(COPY, &from))?;
        if kind.is_symlink() {
            let target = fs::read_link(&from).map_err(at(COPY, &from))?;
            refuse_link(root, &rel, &target).map_err(at(COPY, &from))?;
            symlink(&target, &to, &from).map_err(at(COPY, &to))?;
        } else if kind.is_dir() {
            fs::create_dir(&to).map_err(at(COPY, &to))?;
            copy_tree(root, &from, &to, &rel)?;
        } else if kind.is_file() {
            fs::copy(&from, &to).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("{COPY} {} -> {}: {error}", from.display(), to.display()),
                )
            })?;
        } else {
            return Err(failed(
                COPY,
                &from,
                io::Error::other(verbatim!("not a file, directory or symlink")),
            ));
        }
    }
    Ok(())
}

/// Whether the symlink at `rel` (under `root`) pointing at `target` leaves
/// the package: an absolute target in `root`, or a relative one climbing out
/// of it. One to an absolute path elsewhere doesn't reach package files.
fn reaches_out(root: &Path, rel: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        let canonical = dunce::canonicalize(target).ok();
        // Into the live directory, or to a directory holding it (a path
        // through that reaches it).
        let reaches = |path: &Path| path.starts_with(root) || root.starts_with(path);
        return reaches(target) || canonical.is_some_and(|path| reaches(&path));
    }
    let mut depth = rel.components().count().saturating_sub(1) as i64;
    for part in target.components() {
        match part {
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            // A prefix or root in a relative target: not a package path.
            Component::Prefix(_) | Component::RootDir => return true,
        }
        if depth < 0 {
            return true;
        }
    }
    false
}

/// After the copy: no symlink in it reaches out of it or into the live
/// package, whether or not its destination exists ([`link_escapes`]).
fn check_links(live: &Path, dir: &Path, staged: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir).map_err(at(CHECK, dir))? {
        let entry = entry.map_err(at(CHECK, dir))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(at(CHECK, &path))?;
        if kind.is_dir() {
            check_links(live, &path, staged)?;
        } else if kind.is_symlink()
            && link_escapes(
                staged,
                live,
                dir,
                &fs::read_link(&path).map_err(at(CHECK, &path))?,
            )
        {
            return Err(failed(
                CHECK,
                &path,
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    verbatim!("resolves outside the package"),
                ),
            ));
        }
    }
    Ok(())
}

/// Follows the symlink target `target` from the real directory `dir`, hop by
/// hop as the filesystem would (a `..` after a link applies to where the link
/// leads, a missing end is just appended): where it ends, and whether an
/// absolute link was used on the way. Bounded: `None` for a chain past 40 links
/// or a loop.
pub(crate) fn resolve(dir: &Path, target: &Path) -> Option<(PathBuf, bool)> {
    use std::collections::VecDeque;
    const MAX_LINKS: usize = 40;
    let push = |pending: &mut VecDeque<std::ffi::OsString>, path: &Path| {
        let parts: Vec<_> = path.components().collect();
        for part in parts.into_iter().rev() {
            pending.push_front(part.as_os_str().to_owned());
        }
    };
    let mut resolved = dir.to_owned();
    let mut absolute = false;
    let mut pending = VecDeque::new();
    push(&mut pending, target);
    let mut links = 0;
    while let Some(part) = pending.pop_front() {
        match Path::new(&part).components().next() {
            // A drive or UNC prefix starts the path; the root after it (or a
            // root alone, which keeps the current drive) keeps that prefix.
            Some(Component::Prefix(_)) => {
                resolved = PathBuf::from(&part);
                absolute = true;
            }
            Some(Component::RootDir) => {
                let mut root = PathBuf::new();
                if let Some(Component::Prefix(prefix)) = resolved.components().next() {
                    root.push(prefix.as_os_str());
                }
                root.push(&part);
                resolved = root;
                absolute = true;
            }
            Some(Component::ParentDir) => {
                resolved.pop();
            }
            Some(Component::Normal(name)) => {
                let here = resolved.join(name);
                if fs::symlink_metadata(&here).is_ok_and(|meta| meta.file_type().is_symlink()) {
                    links += 1;
                    if links > MAX_LINKS {
                        return None;
                    }
                    push(&mut pending, &fs::read_link(&here).ok()?);
                } else {
                    resolved = here;
                }
            }
            _ => {}
        }
    }
    Some((resolved, absolute))
}

/// Whether the symlink with target `target`, in the real directory `dir` of
/// the package at `root` (the live directory, or the copy of it), reaches
/// what it mustn't, whether or not where it ends exists:
/// - an absolute target ends in the live directory `live`, or in one that
///   holds it (a path through it would reach the package's mutable files);
/// - a relative target leaves `root`, or gets out through an absolute link.
///
/// Containment is checked on the whole chain, so a missing destination, a
/// `..` or a link along the way is no way round it.
pub(crate) fn link_escapes(root: &Path, live: &Path, dir: &Path, target: &Path) -> bool {
    let Some((end, through_absolute)) = resolve(dir, target) else {
        return true;
    };
    if target.is_absolute() {
        end.starts_with(live) || live.starts_with(&end)
    } else {
        through_absolute || !end.starts_with(root)
    }
}

#[cfg(unix)]
fn symlink(target: &Path, to: &Path, _from: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, to)
}

#[cfg(windows)]
fn symlink(target: &Path, to: &Path, from: &Path) -> io::Result<()> {
    if from.is_dir() {
        std::os::windows::fs::symlink_dir(target, to)
    } else {
        std::os::windows::fs::symlink_file(target, to)
    }
}

/// Clears the write bits of everything under `dir` (the execute bits
/// stay), symlinks left alone, but not of `dir` itself ([`set_readonly`]).
/// An error if any can't be.
fn make_readonly_within(dir: &Path) -> io::Result<()> {
    walk(dir, &mut |path, _| {
        if path == dir {
            return Ok(());
        }
        set_readonly(path)
    })
    .map_err(|error| match error.get_ref() {
        // Already named by the visit.
        Some(_) => error,
        None => failed(PUBLISH, dir, error),
    })
}

/// Clears the write bits of `path` alone (the execute bits stay).
fn set_readonly(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path).map_err(at(PUBLISH, path))?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).map_err(at(PUBLISH, path))
}

/// Undoes [`make_readonly_within`] and [`set_readonly`], so the tree can be
/// removed.
fn make_writable(dir: &Path) {
    let _ = walk(dir, &mut |path, is_dir| {
        let mut permissions = fs::metadata(path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = permissions.mode() | if is_dir { 0o700 } else { 0o200 };
            permissions.set_mode(mode);
        }
        #[cfg(not(unix))]
        {
            let _ = is_dir;
            // Clearing the read-only attribute is the intent; Windows has no mode bits to widen.
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
        }
        fs::set_permissions(path, permissions)
    });
}

/// Visits `root` and everything under it, a directory after what's in it,
/// never through a symlink; the first error stops it.
fn walk(root: &Path, visit: &mut dyn FnMut(&Path, bool) -> io::Result<()>) -> io::Result<()> {
    let meta = fs::symlink_metadata(root)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    let is_dir = meta.is_dir();
    if is_dir {
        // Writable first so it can be listed.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mut permissions = meta.permissions();
            permissions.set_mode(permissions.mode() | 0o700);
            fs::set_permissions(root, permissions)?;
        }
        for entry in fs::read_dir(root)? {
            walk(&entry?.path(), visit)?;
        }
    }
    visit(root, is_dir)
}

/// What a test runs in [`staging_slot`] between opening a staging lock file
/// and locking it, for the store directory it was set for.
#[cfg(test)]
type Hook = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
static AFTER_OPEN: Mutex<Vec<(PathBuf, Hook)>> = Mutex::new(Vec::new());

/// Has `hook` run between the open and the lock in [`staging_slot`] of `dir`.
#[cfg(all(test, unix))]
pub(crate) fn pause_after_open(dir: PathBuf, hook: Hook) {
    AFTER_OPEN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((dir, hook));
}

#[cfg(test)]
fn after_open(dir: &Path) {
    let hook = AFTER_OPEN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|(at, _)| at == dir)
        .map(|(_, hook)| hook.clone());
    if let Some(hook) = hook {
        hook();
    }
}

/// A collection of `dir` that keeps nothing and finds no grants, for tests.
#[cfg(all(test, unix))]
pub(crate) fn collect_unkept(dir: &Path) {
    collect(dir, &State::default());
}

/// Windows paths have drives and UNC prefixes the walk mustn't lose.
#[cfg(all(test, windows))]
mod windows_tests {
    use std::path::Path;

    use super::{link_escapes, resolve};

    #[test]
    fn a_drive_or_unc_prefix_survives_resolution() {
        let (end, absolute) = resolve(
            Path::new(r"C:\state\ext"),
            Path::new(r"C:\state\other\..\ext\future.js"),
        )
        .unwrap();
        assert!(absolute);
        assert_eq!(end, Path::new(r"C:\state\ext\future.js"));
        let (end, _) = resolve(
            Path::new(r"\\server\share\ext"),
            Path::new(r"\\server\share\other\..\ext\x"),
        )
        .unwrap();
        assert_eq!(end, Path::new(r"\\server\share\ext\x"));
    }

    #[test]
    fn a_dangling_absolute_target_into_the_live_package_is_refused() {
        let live = Path::new(r"C:\state\ext");
        assert!(link_escapes(
            live,
            live,
            live,
            Path::new(r"C:\state\other\..\ext\future.js")
        ));
        assert!(!link_escapes(
            live,
            live,
            live,
            Path::new(r"C:\elsewhere\missing")
        ));
    }
}

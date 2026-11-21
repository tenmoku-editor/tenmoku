// SPDX-License-Identifier: Apache-2.0

//! Streamed programs: `process.start` and its `child` resource, for background jobs only (§3.3
//! of `docs/decisions/dqsjfernkt-plugins-stage-3.md`).
//!
//! `start` runs a program as `process.spawn` does (the same grant, bound path, identity check and
//! minimal environment, from [`State::prepare`](crate::process::State)), as a [`ProcessTree`], and
//! returns at once. Its stdout and stderr are each read by a thread of their own into a buffer of
//! at most [`STREAM_BYTES`]: when a buffer is full its thread stops reading, so the program blocks
//! on its next write until the plugin reads, as in a shell pipeline. Its stdin is written by a
//! third thread from a buffer of the same size, which `write` fills only as far as it has room.
//! Plain pipes and blocking threads work the same on every OS, where non-blocking pipes don't.
//!
//! A job holds at most [`MAX_CHILDREN`] children at once. Its first `start` starts a watcher
//! thread, which stops the job at its wall time ([`Stop::TimedOut`]) and, once the job is being
//! stopped, ends every child tree: SIGTERM on Unix, then SIGKILL when the job's
//! [`CANCEL_GRACE`](jobs::CANCEL_GRACE) is up; on Windows the job object is terminated at once
//! ([`ProcessTree::cancel_within`]). When the worker store drops, after the job has ended, what is
//! still running is ended the same way, and the store waits for it: nothing a job started outlives
//! the job.
//!
//! The stream threads end with their streams: at the end of the output (when everything holding
//! it has exited) and once the child is dropped, or the program closes its stdin. Every stream
//! thread of every job counts against [`MAX_STREAM_THREADS`] until it ends, so a program that left
//! its process group on Unix (`setsid`) and still holds a stream keeps its thread, and its count,
//! until it closes it; but `start` refuses past the cap rather than start more.
//!
//! A `cwd` must lie in one of the roots the store's preopens were made from, each pinned when the
//! store is made (`Root::pin`): opened, held open, and resolved, so a root that is a link names
//! the directory it named then, whatever it is pointed at later, and a root that can't be opened
//! is none. The path is rebased onto that directory and resolved there, and checked again just
//! before the launch.
//!
//! That makes `cwd` a convenience checked at start, not a security boundary: the granted program
//! runs unsandboxed with the user's rights and can go anywhere itself. Two windows are known.
//! The roots are pinned apart from the WASI preopens (`fs::configure` opens them by path again),
//! so a root retargeted between the two opens names a different directory here than WASI holds,
//! or stays a root here when its preopen failed; sharing the handle needs a preopen from an open
//! directory in `fs.rs`. And the program starts in its directory by path, so one replaced between
//! the last check and the launch starts it elsewhere; closing that needs a launch into an open
//! directory (`fchdir` before the exec, Unix only) in `ProcessTree`. Windows has no such launch:
//! `CreateProcess` takes the directory as a path.
//!
//! `child.write` is linked by [`add_to_linker`], which lifts its bytes lazily: only what the stdin
//! buffer takes is copied out of the guest's memory.
//!
//! The messages a refusal carries are the API's own, the same in every locale: they aren't
//! translated.

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Write};
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use kiln_command::Grants;
use kiln_command::capability::{Capability, FsScope};
use kiln_core::platform::ProcessTree;
use kiln_l10n::{tr, verbatim};
use wasmtime::StoreContextMut;
use wasmtime::component::{Linker, Resource, WasmList};

use crate::bindings::kiln::extension::process::{Child, Chunk, Exit, HostChild};
use crate::bindings::{Error, HostState};
use crate::fs::Workspace;
use crate::jobs::{self, JobControl, Stop};
use crate::paths::{self, Rules};
use crate::process::SpawnError;

/// The most bytes the host buffers of each of a child's streams (stdout, stderr and stdin), and
/// the most `read` returns of each per call.
pub const STREAM_BYTES: usize = 1 << 20;

/// How many children one job holds at once, counted from `start` until each is dropped.
pub const MAX_CHILDREN: usize = 8;

/// The threads a child's streams take: stdout, stderr and stdin.
pub const THREADS_PER_CHILD: usize = 3;

/// How many stream threads all jobs together may have running: twice what every job running at
/// once ([`jobs::RUNNING_IN_ALL`]) needs at [`MAX_CHILDREN`] each, so threads still waiting on a
/// stream held past its tree leave room.
pub const MAX_STREAM_THREADS: usize = 2 * jobs::RUNNING_IN_ALL * MAX_CHILDREN * THREADS_PER_CHILD;

/// What a stream thread reads or writes at once.
const CHUNK: usize = 64 << 10;

/// `denied`: `process.start` outside a background job.
pub const NOT_IN_A_JOB: &str = verbatim!("process.start runs only in a background job");
/// `failed`: past [`MAX_CHILDREN`].
pub const TOO_MANY_CHILDREN: &str = verbatim!("too many programs started in this job");
/// `failed`: past [`MAX_STREAM_THREADS`].
pub const TOO_MANY_STREAMS: &str = verbatim!("too many program streams open");
/// `failed`: no thread could be started for the program's streams, or for the job's watcher.
pub const NO_THREAD: &str = verbatim!("no thread for a program's streams");
/// `failed`: `write` once stdin is closed, by `close-stdin` or by the program.
pub const STDIN_CLOSED: &str = verbatim!("stdin is closed");
/// `not-found`: a child this store doesn't hold.
pub const NO_SUCH_CHILD: &str = verbatim!("no such child");

/// Stream threads running now.
static STREAM_THREADS: AtomicUsize = AtomicUsize::new(0);

/// How many stream threads are running now, of every job: each ends with its stream.
pub fn stream_threads() -> usize {
    STREAM_THREADS.load(Ordering::Acquire)
}

/// The roots of `workspace` that `grants` preopen for a store (as `fs::configure` does): every
/// root with a guest path, given `fs.read:workspace` or `fs.write:workspace`, else none. What
/// [`crate::process::State::with_roots`] takes.
pub fn preopened(grants: &Grants, workspace: &Workspace) -> Vec<PathBuf> {
    let reaches = grants.allows(&Capability::FsRead(FsScope::Workspace))
        || grants.allows(&Capability::FsWrite(FsScope::Workspace));
    if !reaches {
        return Vec::new();
    }
    let has_guest_path = |root: &&PathBuf| {
        root.to_str()
            .is_some_and(|text| paths::root_to_guest(text, Rules::HOST).is_ok())
    };
    workspace
        .roots
        .iter()
        .filter(has_guest_path)
        .cloned()
        .collect()
}

/// The `process.start` slice of a store's process state: the children it holds, and the roots a
/// `cwd` may name. Dropped, it ends what is still running, and waits for that.
#[derive(Default)]
pub struct Streams {
    roots: Vec<Root>,
    /// By resource rep.
    children: BTreeMap<u32, Arc<Shared>>,
    next: u32,
    /// The job's watcher, from its first `start`.
    watch: Option<Watching>,
}

/// The watcher thread, and what it shares with the store.
struct Watching {
    watch: Arc<Watch>,
    thread: Option<JoinHandle<()>>,
}

struct Watch {
    control: JobControl,
    /// When the job's wall time ends, if it has one.
    wall_ends: Option<Instant>,
    state: Mutex<WatchState>,
    wake: Condvar,
}

#[derive(Default)]
struct WatchState {
    children: Vec<Arc<Shared>>,
    /// The job is being stopped: its trees are being ended, and no child is added.
    stopping: bool,
    /// The store dropped: the watcher returns.
    dropped: bool,
}

/// A root a `cwd` may lie in, pinned as it was when the store was made: what it named then is
/// what it names, whatever its path names since.
struct Root {
    /// As the app gave it: what its preopen's guest path is made from.
    given: String,
    /// The directory it named, its links resolved.
    canonical: PathBuf,
    /// That directory's identity.
    identity: (u64, u64),
    /// That directory, held open so its identity can't pass to another.
    _held: std::fs::File,
}

impl Root {
    /// `root` pinned now: none, with a warning, if it can't be opened as a directory, as when its
    /// preopen failed too, or it changed while it was pinned.
    fn pin(root: &Path) -> Option<Self> {
        let pinned = (|| {
            let given = root
                .to_str()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
            let held = kiln_core::platform::open_dir(root)?;
            let identity = kiln_core::platform::file_identity(&held)?;
            let canonical = kiln_core::platform::canonicalize(root)?;
            if kiln_core::platform::path_identity(&canonical, true)? != identity {
                return Err(io::Error::from(io::ErrorKind::NotFound));
            }
            Ok::<_, io::Error>(Self {
                given: given.to_owned(),
                canonical,
                identity,
                _held: held,
            })
        })();
        pinned
            .inspect_err(|error| {
                log::warn!("no process.start in {}: {error}", root.display());
            })
            .ok()
    }

    /// Whether its path still names the directory it was pinned to.
    fn unchanged(&self) -> bool {
        kiln_core::platform::path_identity(&self.canonical, true)
            .is_ok_and(|identity| identity == self.identity)
    }
}

/// A `cwd` checked: the directory, its links resolved, inside root `root`, and its identity, to
/// check again just before the launch.
struct Cwd {
    guest: String,
    path: PathBuf,
    identity: (u64, u64),
    root: usize,
}

impl Streams {
    /// Pins `roots` (see [`Root::pin`]): each one that opens as a directory.
    pub(crate) fn set_roots(&mut self, roots: Vec<PathBuf>) {
        self.roots = roots.iter().filter_map(|root| Root::pin(root)).collect();
    }

    /// How many children it holds.
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Guest path `guest` as the directory to start in: inside one of the pinned roots by its
    /// words, then rebased onto the directory that root was pinned to and resolved there, which
    /// must lie inside it still.
    fn cwd(&self, guest: &str) -> Result<Cwd, Error> {
        let denied = || {
            let why = tr!(
                "kiln-plugin-path-outside-roots",
                path = format!("{guest:?}")
            );
            Error::Denied(why.to_string())
        };
        let given = self.roots.iter().map(|root| root.given.as_str());
        let host = paths::to_host_within(guest, Rules::HOST, given)?;
        let normal = paths::to_guest(&host, Rules::HOST).map_err(|_| denied())?;
        // The root it lies in by its words, the longest if roots nest, and what follows it.
        let mut found: Option<(usize, String, usize)> = None;
        for (index, root) in self.roots.iter().enumerate() {
            let Ok(prefix) = paths::root_to_guest(&root.given, Rules::HOST) else {
                continue;
            };
            let prefix = prefix.trim_end_matches('/');
            let Some(rest) = normal.strip_prefix(prefix) else {
                continue;
            };
            let inside = rest.is_empty() || rest.starts_with('/');
            if inside && found.as_ref().is_none_or(|(_, _, len)| prefix.len() > *len) {
                found = Some((index, rest.to_owned(), prefix.len()));
            }
        }
        let (index, rest, _) = found.ok_or_else(denied)?;
        let root = &self.roots[index];
        if !root.unchanged() {
            return Err(denied());
        }
        let mut candidate = root.canonical.clone();
        candidate.extend(rest.split('/').filter(|part| !part.is_empty()));
        // The guest knows the path it named: the reason is the OS's.
        let not_found = |error: io::Error| Error::NotFound(error.to_string());
        let path = kiln_core::platform::canonicalize(&candidate).map_err(not_found)?;
        if !path.starts_with(&root.canonical) {
            return Err(denied());
        }
        let identity = kiln_core::platform::path_identity(&path, true).map_err(not_found)?;
        Ok(Cwd {
            guest: guest.to_owned(),
            path,
            identity,
            root: index,
        })
    }

    /// Checks `cwd` again, just before the launch: its root and its path still name the
    /// directories they named when it was checked, so nothing replaced since leads out.
    fn recheck(&self, cwd: &Cwd) -> Result<(), Error> {
        let same = self.roots.get(cwd.root).is_some_and(Root::unchanged)
            && kiln_core::platform::canonicalize(&cwd.path).is_ok_and(|path| path == cwd.path)
            && kiln_core::platform::path_identity(&cwd.path, true)
                .is_ok_and(|identity| identity == cwd.identity);
        if same {
            return Ok(());
        }
        let why = tr!(
            "kiln-plugin-path-outside-roots",
            path = format!("{:?}", cwd.guest)
        );
        Err(Error::Denied(why.to_string()))
    }

    /// Starts the job's watcher, if it has none yet.
    fn watching(&mut self, control: &JobControl, wall_ends: Option<Instant>) -> Result<(), Error> {
        if self.watch.is_some() {
            return Ok(());
        }
        let watch = Arc::new(Watch {
            control: control.clone(),
            wall_ends,
            state: Mutex::default(),
            wake: Condvar::new(),
        });
        let thread = thread::Builder::new()
            .name("plugin-stream-watch".into())
            .spawn({
                let watch = Arc::clone(&watch);
                move || watch_job(&watch)
            })
            .map_err(|_| Error::Failed(NO_THREAD.to_owned()))?;
        self.watch = Some(Watching {
            watch,
            thread: Some(thread),
        });
        Ok(())
    }

    /// Holds `child`, and returns its rep: none once the job is being stopped.
    fn add(&mut self, child: &Arc<Shared>) -> Option<u32> {
        let watching = self.watch.as_ref()?;
        let mut state = lock(&watching.watch.state);
        if state.stopping {
            return None;
        }
        state.children.push(Arc::clone(child));
        drop(state);
        let rep = loop {
            let rep = self.next;
            self.next = self.next.wrapping_add(1);
            if !self.children.contains_key(&rep) {
                break rep;
            }
        };
        self.children.insert(rep, Arc::clone(child));
        Some(rep)
    }

    fn child(&self, child: &Resource<Child>) -> Result<Arc<Shared>, Error> {
        (self.children.get(&child.rep()).cloned())
            .ok_or_else(|| Error::NotFound(NO_SUCH_CHILD.to_owned()))
    }

    fn remove(&mut self, rep: u32) -> Option<Arc<Shared>> {
        let child = self.children.remove(&rep)?;
        if let Some(watching) = &self.watch {
            let mut state = lock(&watching.watch.state);
            state.children.retain(|held| !Arc::ptr_eq(held, &child));
        }
        Some(child)
    }
}

impl Drop for Streams {
    /// The job has ended: the watcher stops, after ending the trees if the job was being stopped,
    /// and what is still running is ended as on a cancel, the rest of the grace given to it, or
    /// all of [`ProcessTree::GRACE`] after a job that ended by itself.
    fn drop(&mut self) {
        let Some(mut watching) = self.watch.take() else {
            return;
        };
        {
            let mut state = lock(&watching.watch.state);
            state.dropped = true;
            state.stopping = true;
        }
        watching.watch.wake.notify_all();
        if let Some(thread) = watching.thread.take() {
            drop(thread.join());
        }
        let children: Vec<_> = mem::take(&mut self.children).into_values().collect();
        end_all(&children, grace_left(&watching.watch.control));
        for child in &children {
            child.close();
        }
    }
}

/// The watcher: stops the job at its wall time, and once it is being stopped ends every child's
/// tree, then returns. Returns too once the store drops.
fn watch_job(watch: &Watch) {
    let mut state = lock(&watch.state);
    loop {
        if state.dropped {
            return;
        }
        if watch.wall_ends.is_some_and(|ends| Instant::now() >= ends) {
            watch.control.stop(Stop::TimedOut);
        }
        if watch.control.is_cancelled() {
            break;
        }
        state = (watch.wake.wait_timeout(state, jobs::STOP_POLL))
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
    state.stopping = true;
    let children = state.children.clone();
    drop(state);
    end_all(&children, grace_left(&watch.control));
}

/// What is left of the grace of a job being stopped, or all of [`ProcessTree::GRACE`] if it
/// isn't.
fn grace_left(control: &JobControl) -> Duration {
    match control.ends(None) {
        Some(ends) => ends.saturating_duration_since(Instant::now()),
        None => ProcessTree::GRACE,
    }
}

/// Ends every one of `children`'s trees at once, each given `grace`, and returns once they have
/// all ended.
fn end_all(children: &[Arc<Shared>], grace: Duration) {
    thread::scope(|scope| {
        for child in children {
            let ending = thread::Builder::new()
                .name("plugin-stream-end".into())
                .spawn_scoped(scope, || child.end_within(grace));
            // No thread: no grace either.
            if ending.is_err() {
                child.kill();
            }
        }
    });
}

/// What a child's handle, its stream threads and the watcher share.
struct Shared {
    io: Mutex<Io>,
    /// Signalled on every change to `io`: output came or was taken, input was given or written,
    /// a stream ended, the program exited.
    changed: Condvar,
    tree: Mutex<Tree>,
}

#[derive(Default)]
struct Io {
    stdout: Output,
    stderr: Output,
    stdin: Input,
    /// The child was dropped: its threads stop, and its buffers are emptied.
    dropped: bool,
}

#[derive(Default)]
struct Output {
    bytes: VecDeque<u8>,
    /// Its end was read, or reading it failed.
    ended: bool,
}

#[derive(Default)]
struct Input {
    bytes: VecDeque<u8>,
    /// What its thread took from `bytes` and is writing now: still held, so still counted.
    writing: usize,
    /// `close-stdin` was called: closed once `bytes` are written.
    closing: bool,
    /// Its thread has closed it, or the program did.
    closed: bool,
}

impl Io {
    fn output(&mut self, stream: Stream) -> &mut Output {
        match stream {
            Stream::Stdout => &mut self.stdout,
            Stream::Stderr => &mut self.stderr,
        }
    }
}

#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

struct Tree {
    /// None once it is being ended, or has been.
    running: Option<ProcessTree>,
    exit: Option<Exit>,
}

impl Shared {
    /// How the program ended, if it has, looked at now.
    fn exit(&self) -> Option<Exit> {
        let mut tree = lock(&self.tree);
        if tree.exit.is_none()
            && let Some(running) = &mut tree.running
            && let Ok(Some(status)) = running.try_wait()
        {
            tree.exit = Some(exit(Ok(status), false));
        }
        tree.exit
    }

    /// Kills the tree at once, if it still runs.
    fn kill(&self) {
        let running = lock(&self.tree).running.take();
        if let Some(running) = running {
            self.ended(running, |tree| tree.kill());
        }
    }

    /// Ends the tree, giving it `grace` first, if it still runs.
    fn end_within(&self, grace: Duration) {
        let running = lock(&self.tree).running.take();
        if let Some(running) = running {
            self.ended(running, |tree| tree.cancel_within(grace));
        }
    }

    /// Ends `running` by `end`, and records how: as the program's own exit if it had exited.
    fn ended(
        &self,
        mut running: ProcessTree,
        end: impl FnOnce(&mut ProcessTree) -> io::Result<ExitStatus>,
    ) {
        let exited = running.try_wait().ok().flatten();
        let status = end(&mut running);
        let ended = match exited {
            Some(status) => exit(Ok(status), false),
            None => exit(status, true),
        };
        lock(&self.tree).exit.get_or_insert(ended);
        self.changed.notify_all();
    }

    /// The child is dropped: its threads stop, and what it buffered goes.
    fn close(&self) {
        let mut io = lock(&self.io);
        io.dropped = true;
        io.stdout.bytes = VecDeque::new();
        io.stderr.bytes = VecDeque::new();
        io.stdin.bytes = VecDeque::new();
        drop(io);
        self.changed.notify_all();
    }

    /// `child.read`: waits until `until` for output, or the end, and takes at most `max` bytes
    /// of each stream. In a job (`control`), fails as cancelled once the job is being stopped,
    /// or past `ends`, when it must end.
    fn read(
        &self,
        max: usize,
        until: Option<Instant>,
        control: Option<&JobControl>,
        ends: Option<Instant>,
    ) -> Result<Chunk, Error> {
        loop {
            if let Some(control) = control {
                // An import that waited to the job's end doesn't make its wall time longer.
                if ends.is_some_and(|ends| Instant::now() >= ends) {
                    control.stop(Stop::TimedOut);
                }
                if control.is_cancelled() {
                    return Err(Error::Failed(jobs::CANCELLED.to_owned()));
                }
            }
            let exit = self.exit();
            let mut io = lock(&self.io);
            let any = !io.stdout.bytes.is_empty() || !io.stderr.bytes.is_empty();
            let drained = io.stdout.ended && io.stderr.ended;
            let now = Instant::now();
            let waited = until.is_some_and(|until| now >= until);
            if any || (exit.is_some() && drained) || waited || max == 0 {
                let stdout = take(&mut io.stdout.bytes, max);
                let stderr = take(&mut io.stderr.bytes, max);
                let empty = io.stdout.bytes.is_empty() && io.stderr.bytes.is_empty();
                drop(io);
                if !stdout.is_empty() || !stderr.is_empty() {
                    // Room for the stream threads to read on.
                    self.changed.notify_all();
                }
                return Ok(Chunk {
                    stdout,
                    stderr,
                    exited: exit.filter(|_| drained && empty),
                });
            }
            let wait = until.map_or(jobs::STOP_POLL, |until| (until - now).min(jobs::STOP_POLL));
            drop(self.changed.wait_timeout(io, wait));
        }
    }

    /// `child.write`: takes what fits of `bytes`.
    fn write(&self, bytes: &[u8]) -> Result<u64, Error> {
        let mut io = lock(&self.io);
        let stdin = &mut io.stdin;
        if stdin.closing || stdin.closed {
            return Err(Error::Failed(STDIN_CLOSED.to_owned()));
        }
        let room = STREAM_BYTES.saturating_sub(stdin.bytes.len() + stdin.writing);
        let taken = room.min(bytes.len());
        stdin.bytes.extend(&bytes[..taken]);
        drop(io);
        if taken > 0 {
            self.changed.notify_all();
        }
        Ok(u64::try_from(taken).unwrap_or(u64::MAX))
    }

    fn close_stdin(&self) {
        lock(&self.io).stdin.closing = true;
        self.changed.notify_all();
    }
}

/// How `status` ended a program, `killed` if the host ended it. A status that couldn't be read
/// has no code.
fn exit(status: io::Result<ExitStatus>, killed: bool) -> Exit {
    Exit {
        code: status.ok().and_then(|status| status.code()),
        killed,
    }
}

/// At most `max` bytes off the front of `bytes`.
fn take(bytes: &mut VecDeque<u8>, max: usize) -> Vec<u8> {
    let count = bytes.len().min(max);
    bytes.drain(..count).collect()
}

/// Room reserved for stream threads under [`MAX_STREAM_THREADS`]: each thread started takes one
/// [`Slot`] of it, and what isn't taken goes back when this drops.
struct Threads(usize);

impl Threads {
    fn reserve(count: usize) -> Option<Self> {
        (STREAM_THREADS.fetch_update(Ordering::AcqRel, Ordering::Acquire, |running| {
            (running + count <= MAX_STREAM_THREADS).then_some(running + count)
        }))
        .ok()
        .map(|_| Self(count))
    }

    /// Starts `work` on a thread of its own named `name`, which holds a slot until it returns.
    fn spawn(&mut self, name: &str, work: impl FnOnce() + Send + 'static) -> io::Result<()> {
        if self.0 == 0 {
            return Err(io::Error::other(TOO_MANY_STREAMS));
        }
        self.0 -= 1;
        let slot = Slot;
        thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                kiln_core::platform::lower_thread_priority();
                work();
                drop(slot);
            })
            .map(drop)
    }
}

impl Drop for Threads {
    fn drop(&mut self) {
        STREAM_THREADS.fetch_sub(self.0, Ordering::AcqRel);
    }
}

/// One stream thread's place under [`MAX_STREAM_THREADS`], given back when it drops.
struct Slot;

impl Drop for Slot {
    fn drop(&mut self) {
        STREAM_THREADS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Reads `pipe` into `stream`'s buffer while it has room, until its end or the child drops.
fn drain(shared: &Shared, mut pipe: impl Read, stream: Stream) {
    let mut chunk = vec![0; CHUNK];
    loop {
        let room = {
            let mut io = lock(&shared.io);
            loop {
                if io.dropped {
                    return;
                }
                let room = STREAM_BYTES.saturating_sub(io.output(stream).bytes.len());
                if room > 0 {
                    break room;
                }
                io = shared
                    .changed
                    .wait(io)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let read = loop {
            match pipe.read(&mut chunk[..room.min(CHUNK)]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                read => break read,
            }
        };
        let mut io = lock(&shared.io);
        if io.dropped {
            return;
        }
        let output = io.output(stream);
        let more = match read {
            Ok(0) | Err(_) => {
                output.ended = true;
                false
            }
            Ok(read) => {
                output.bytes.extend(&chunk[..read]);
                true
            }
        };
        drop(io);
        shared.changed.notify_all();
        if !more {
            return;
        }
    }
}

/// Writes what `write` gave into `pipe`, closing it once `close-stdin` was called and all is
/// written, or once the program closes its end, or the child drops.
fn feed(shared: &Shared, mut pipe: impl Write) {
    let mut chunk = Vec::with_capacity(CHUNK);
    loop {
        {
            let mut io = lock(&shared.io);
            loop {
                if io.dropped {
                    return;
                }
                let stdin = &mut io.stdin;
                if !stdin.bytes.is_empty() {
                    let count = stdin.bytes.len().min(CHUNK);
                    chunk.extend(stdin.bytes.drain(..count));
                    stdin.writing = count;
                    break;
                }
                if stdin.closing {
                    stdin.closed = true;
                    drop(io);
                    shared.changed.notify_all();
                    // Dropping the pipe closes it.
                    return;
                }
                io = shared
                    .changed
                    .wait(io)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        }
        let written = pipe.write_all(&chunk).and_then(|()| pipe.flush());
        chunk.clear();
        let mut io = lock(&shared.io);
        io.stdin.writing = 0;
        let broken = written.is_err();
        if broken {
            io.stdin.closed = true;
            io.stdin.bytes = VecDeque::new();
        }
        drop(io);
        shared.changed.notify_all();
        if broken {
            return;
        }
    }
}

/// `process.start` for `host`'s store: refused outside a job, past [`MAX_CHILDREN`] or
/// [`MAX_STREAM_THREADS`], for a `cwd` outside the roots, and as `spawn` refuses.
pub(crate) fn start(
    host: &mut HostState,
    program: &str,
    args: &[String],
    cwd: Option<&str>,
) -> Result<Resource<Child>, Error> {
    let Some(control) = host.jobs.control().cloned() else {
        return Err(Error::Denied(NOT_IN_A_JOB.to_owned()));
    };
    let cancelled = || Error::Failed(jobs::CANCELLED.to_owned());
    if control.is_cancelled() {
        return Err(cancelled());
    }
    let streams = &mut host.process.streams;
    if streams.children.len() >= MAX_CHILDREN {
        return Err(Error::Failed(TOO_MANY_CHILDREN.to_owned()));
    }
    let cwd = cwd.map(|cwd| streams.cwd(cwd)).transpose()?;
    let mut threads = Threads::reserve(THREADS_PER_CHILD)
        .ok_or_else(|| Error::Failed(TOO_MANY_STREAMS.to_owned()))?;
    // Entering the host, a job's deadline is when it must end: its wall time, as it isn't being
    // stopped yet.
    streams.watching(&control, host.deadline)?;
    let remaining = host.program_runs.remaining();
    let prepared = (host.process)
        .prepare(program, args, remaining, Instant::now(), Some(&control))
        .map_err(SpawnError::into_error)?;
    let mut command = prepared.command;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let failed = |source: io::Error| {
        SpawnError::Io {
            path: prepared.path.clone(),
            source,
        }
        .into_error()
    };
    // A job stopped while the program was checked starts nothing.
    if control.is_cancelled() {
        return Err(cancelled());
    }
    // Checked again as late as can be: a directory replaced while the program was checked
    // doesn't lead out.
    if let Some(cwd) = &cwd {
        host.process.streams.recheck(cwd)?;
        command.current_dir(&cwd.path);
    }
    let mut tree = ProcessTree::spawn(&mut command).map_err(failed)?;
    // The command holds the program's ends of the pipes: they must close here for its output to
    // end.
    drop(command);
    let pipes = (tree.take_stdin(), tree.take_stdout(), tree.take_stderr());
    let shared = Arc::new(Shared {
        io: Mutex::default(),
        changed: Condvar::new(),
        tree: Mutex::new(Tree {
            running: Some(tree),
            exit: None,
        }),
    });
    let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
        shared.kill();
        return Err(Error::Failed(NO_THREAD.to_owned()));
    };
    let out = Arc::clone(&shared);
    let err = Arc::clone(&shared);
    let input = Arc::clone(&shared);
    let started = threads
        .spawn("plugin-stream-stdout", move || {
            drain(&out, stdout, Stream::Stdout);
        })
        .and_then(|()| {
            threads.spawn("plugin-stream-stderr", move || {
                drain(&err, stderr, Stream::Stderr);
            })
        })
        .and_then(|()| threads.spawn("plugin-stream-stdin", move || feed(&input, stdin)));
    if started.is_err() {
        shared.kill();
        shared.close();
        return Err(Error::Failed(NO_THREAD.to_owned()));
    }
    // A job stopped since the program started ends it here: the watcher may have ended the rest.
    let Some(rep) = host.process.streams.add(&shared) else {
        shared.kill();
        shared.close();
        return Err(cancelled());
    };
    Ok(Resource::new_own(rep))
}

/// The `process` interface's name, as the current world imports it.
const PROCESS: &str = "kiln:extension/process@1.1.0";

/// Defines `child.write` in `linker` over what [`crate::bindings::add_to_linker`] defined: its
/// bytes are lifted lazily, so only what the stdin buffer takes is copied out of the guest's
/// memory, never the whole list a guest passes. [`HostChild::write`] would get the list copied
/// whole into the host first, past the memory the store's limiter holds the guest to.
pub fn add_to_linker<T: 'static>(
    linker: &mut Linker<T>,
    get: fn(&mut T) -> &mut HostState,
) -> wasmtime::Result<()> {
    linker.allow_shadowing(true);
    let defined = linker.instance(PROCESS).and_then(|mut process| {
        process.func_wrap(
            "[method]child.write",
            move |mut store: StoreContextMut<'_, T>,
                  (child, bytes): (Resource<Child>, WasmList<u8>)| {
                let child = get(store.data_mut()).process.streams.child(&child);
                let written = child.and_then(|child| child.write(bytes.as_le_slice(&store)));
                Ok((written,))
            },
        )
    });
    linker.allow_shadowing(false);
    defined
}

impl HostChild for HostState {
    fn read(
        &mut self,
        child: Resource<Child>,
        max_bytes: u32,
        timeout_ms: u32,
    ) -> Result<Chunk, Error> {
        let child = self.process.streams.child(&child)?;
        let max = usize::try_from(max_bytes)
            .unwrap_or(usize::MAX)
            .min(STREAM_BYTES);
        let until = Instant::now().checked_add(Duration::from_millis(timeout_ms.into()));
        // No later than the job must end.
        let until = match (until, self.deadline) {
            (Some(until), Some(ends)) => Some(until.min(ends)),
            (until, ends) => until.or(ends),
        };
        child.read(max, until, self.jobs.control(), self.deadline)
    }

    /// Not linked by the runner, which defines [`add_to_linker`]'s in its place.
    fn write(&mut self, child: Resource<Child>, bytes: Vec<u8>) -> Result<u64, Error> {
        self.process.streams.child(&child)?.write(&bytes)
    }

    fn close_stdin(&mut self, child: Resource<Child>) {
        if let Ok(child) = self.process.streams.child(&child) {
            child.close_stdin();
        }
    }

    fn kill(&mut self, child: Resource<Child>) {
        if let Ok(child) = self.process.streams.child(&child) {
            child.kill();
        }
    }

    /// Dropping a child kills its tree at once, as `kill` does, and stops its threads.
    fn drop(&mut self, child: Resource<Child>) -> wasmtime::Result<()> {
        if let Some(child) = self.process.streams.remove(child.rep()) {
            child.kill();
            child.close();
        }
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// A directory under the system's temporary one, empty.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kiln-plugin-stream-{}-{name}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        kiln_core::platform::canonicalize(&dir).unwrap()
    }

    fn guest(path: &Path) -> String {
        paths::to_guest(path.to_str().unwrap(), Rules::HOST).unwrap()
    }

    #[test]
    fn cwd_replaced_before_the_launch_is_refused() {
        let dir = scratch("replaced");
        let root = dir.join("root");
        let cwd = root.join("work");
        fs::create_dir_all(&cwd).unwrap();
        let mut streams = Streams::default();
        streams.set_roots(vec![root.clone()]);
        let checked = streams.cwd(&guest(&cwd)).unwrap();
        assert_eq!(checked.path, cwd);
        assert!(streams.recheck(&checked).is_ok());

        // Another directory put in its place since it was checked.
        fs::rename(&cwd, dir.join("moved")).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        assert!(matches!(streams.recheck(&checked), Err(Error::Denied(_))));

        // The root itself replaced: nothing in it is a place to start any more.
        let checked = streams.cwd(&guest(&root)).unwrap();
        fs::rename(&root, dir.join("old-root")).unwrap();
        fs::create_dir_all(&root).unwrap();
        assert!(matches!(streams.recheck(&checked), Err(Error::Denied(_))));
        assert!(matches!(streams.cwd(&guest(&root)), Err(Error::Denied(_))));
        fs::remove_dir_all(&dir).ok();
    }
}

//! `RemoteHost` as `FsHost` (D § 5, § 7, § 12): a tenmoku-server session
//! in this process, over the injected-latency `Delayed` link, answers as
//! the local host does on the same files; a read whose link drops runs once
//! more on the next connection, and a write never does.

#![cfg(unix)]

use fe_host::{DiskStamp, FsHost, ListRequest, LocalFs, TrashRecord, WriteError, WriteExpect};
use fe_project::LocalTreeFs;
use fe_remote_client::{Conn, Connection, LinkError, RemoteHost};
use fe_remote_proto::testing::{Latency, delayed};
use fe_remote_proto::window::INITIAL_WINDOW;
use fe_remote_proto::{Config as MuxConfig, Location, Mux, PathBytes};
use std::ffi::OsString;
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tenmoku_server::{Config, Hosts};

/// The latency the parity tests run at: enough to be a network, little
/// enough for many calls.
const ONE_WAY: Duration = Duration::from_millis(2);

/// A session served in this process for `hosts`, its client's link
/// delayed by `one_way` each way. `cut` drops the link.
struct Served {
    connection: Connection,
    cut: UnixStream,
}

impl Served {
    fn new(hosts: Hosts, one_way: Duration) -> Served {
        let (client, server) = UnixStream::pair().unwrap();
        let config = Config {
            hosts: Arc::new(move || hosts.clone()),
            ..Config::default()
        };
        let server = tenmoku_server::socket_link(server).unwrap();
        thread::spawn(move || tenmoku_server::serve(server, config));
        let cut = client.try_clone().unwrap();
        let link = delayed(
            tenmoku_server::socket_link(client).unwrap(),
            Latency::one_way(one_way),
        );
        let config = MuxConfig {
            heartbeat: None,
            window: INITIAL_WINDOW,
            ..MuxConfig::default()
        };
        let location = Location {
            destination: "test".into(),
            path: PathBytes(b"/".to_vec()),
        };
        let (mux, welcome) = Mux::client(link, config, location, None).unwrap();
        Served {
            connection: Connection::over(mux, welcome),
            cut,
        }
    }
}

/// A remote host over a session of this machine's hosts.
fn remote() -> RemoteHost {
    RemoteHost::new(Conn::new(Served::new(Hosts::local(), ONE_WAY).connection))
}

/// A project folder, by its canonical path: the tree reaches its entries
/// without following symlinks, and a temp dir may sit behind one.
struct Project {
    _dir: TempDir,
    root: PathBuf,
}

impl Project {
    fn new() -> Project {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        Project { _dir: dir, root }
    }

    /// The files every test starts from.
    fn fixture() -> Project {
        let project = Project::new();
        project.write("a.txt", "alpha\n");
        project.write("src/main.rs", "fn main() {}\n");
        project.write("src/lib.rs", "");
        project.write("target/out.o", "obj");
        project.write(".gitignore", "target/\n");
        std::os::unix::fs::symlink("src/main.rs", project.at("link")).unwrap();
        std::os::unix::fs::symlink("nowhere", project.at("dangling")).unwrap();
        project
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.at(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    /// Every entry under the root, relative to it, with what it is: the
    /// contents of a file, the target of a symlink.
    fn snapshot(&self) -> Vec<(PathBuf, String)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let rel = path.strip_prefix(root).unwrap().to_path_buf();
                let meta = std::fs::symlink_metadata(&path).unwrap();
                if meta.is_symlink() {
                    let to = std::fs::read_link(&path).unwrap();
                    out.push((rel, format!("-> {}", to.display())));
                } else if meta.is_dir() {
                    out.push((rel, "/".into()));
                    walk(root, &path, out);
                } else {
                    out.push((rel, std::fs::read_to_string(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &self.root, &mut out);
        out.sort();
        out
    }

    fn rel(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.root).unwrap().to_path_buf()
    }
}

fn kind<T: std::fmt::Debug>(result: io::Result<T>) -> io::ErrorKind {
    result.unwrap_err().kind()
}

fn dev_ino(path: &Path) -> (u64, u64) {
    let meta = std::fs::symlink_metadata(path).unwrap();
    (meta.dev(), meta.ino())
}

#[test]
fn read_matches_the_local_host() {
    let (host, project) = (remote(), Project::fixture());
    for rel in ["a.txt", "src/lib.rs", "link"] {
        let path = project.at(rel);
        assert_eq!(
            host.read(&path).unwrap(),
            LocalFs.read(&path).unwrap(),
            "{rel}"
        );
    }
    let missing = project.at("missing");
    assert_eq!(kind(host.read(&missing)), kind(LocalFs.read(&missing)));
    let dir = project.at("src");
    assert_eq!(kind(host.read(&dir)), kind(LocalFs.read(&dir)));
}

#[test]
fn a_read_of_several_chunks_comes_whole() {
    let (host, project) = (remote(), Project::new());
    let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let path = project.at("big.bin");
    std::fs::write(&path, &bytes).unwrap();
    let (read, stamp) = host.read(&path).unwrap();
    assert_eq!(read, bytes);
    assert_eq!(Some(stamp), DiskStamp::of(&path).unwrap());
}

#[test]
fn stat_matches_the_local_host() {
    let (host, project) = (remote(), Project::fixture());
    for rel in ["a.txt", "src", "link", "dangling", "missing"] {
        let path = project.at(rel);
        assert_eq!(
            host.stat(&path).unwrap(),
            LocalFs.stat(&path).unwrap(),
            "{rel}"
        );
    }
}

#[test]
fn list_dir_matches_the_local_host() {
    let (host, project) = (remote(), Project::fixture());
    let sorted = |mut entries: Vec<fe_host::DirEntry>| {
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    };
    for rel in ["", "src"] {
        let path = project.at(rel);
        assert_eq!(
            sorted(host.list_dir(&path).unwrap()),
            sorted(LocalFs.list_dir(&path).unwrap()),
            "{rel:?}"
        );
    }
    let file = project.at("a.txt");
    assert_eq!(kind(host.list_dir(&file)), kind(LocalFs.list_dir(&file)));
}

#[test]
fn canonicalize_matches_the_local_host() {
    let (host, project) = (remote(), Project::fixture());
    for rel in ["link", "src/../a.txt", "."] {
        let path = project.at(rel);
        assert_eq!(
            host.canonicalize(&path).unwrap(),
            LocalFs.canonicalize(&path).unwrap(),
            "{rel}"
        );
    }
    let dangling = project.at("dangling");
    assert_eq!(
        kind(host.canonicalize(&dangling)),
        kind(LocalFs.canonicalize(&dangling))
    );
}

#[test]
fn a_write_returns_the_new_files_stamp() {
    let (host, project) = (remote(), Project::fixture());
    let path = project.at("a.txt");
    let stamp = DiskStamp::of(&path).unwrap().unwrap();
    let written = host
        .write_atomic(&path, b"beta\n", WriteExpect::Matches(stamp))
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"beta\n");
    assert_eq!(Some(written), DiskStamp::of(&path).unwrap());
}

#[test]
fn a_write_of_several_chunks_lands_whole() {
    let (host, project) = (remote(), Project::new());
    let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 253) as u8).collect();
    let path = project.at("new.bin");
    host.write_atomic(&path, &bytes, WriteExpect::Absent)
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn a_matches_write_over_a_changed_file_conflicts_with_its_current_stamp() {
    let (host, project) = (remote(), Project::fixture());
    let path = project.at("a.txt");
    let read = DiskStamp::of(&path).unwrap().unwrap();
    std::fs::write(&path, "changed on the host, and longer\n").unwrap();
    let current = DiskStamp::of(&path).unwrap();
    match host.write_atomic(&path, b"mine\n", WriteExpect::Matches(read)) {
        Err(WriteError::Conflict(conflict)) => assert_eq!(conflict.current, current),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "changed on the host, and longer\n"
    );
}

#[test]
fn an_absent_write_over_a_file_conflicts_with_its_current_stamp() {
    let (host, project) = (remote(), Project::fixture());
    let path = project.at("a.txt");
    let current = DiskStamp::of(&path).unwrap();
    match host.write_atomic(&path, b"mine\n", WriteExpect::Absent) {
        Err(WriteError::Conflict(conflict)) => assert_eq!(conflict.current, current),
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\n");
}

#[test]
fn a_write_the_host_refuses_fails_with_its_error() {
    let (host, project) = (remote(), Project::fixture());
    let path = project.at("no/such/dir/file");
    let local = LocalFs.write_atomic(&path, b"x", WriteExpect::Overwrite);
    match (
        host.write_atomic(&path, b"x", WriteExpect::Overwrite),
        local,
    ) {
        (Err(WriteError::Io(remote)), Err(WriteError::Io(local))) => {
            assert_eq!(remote.kind(), local.kind());
        }
        other => panic!("{other:?}"),
    }
}

fn list_request(project: &Project, rel: &str, show_ignored: bool) -> ListRequest {
    let depth = Path::new(rel).components().count();
    ListRequest {
        root: project.root.clone(),
        rel: rel.into(),
        show_ignored,
        parent_ignored: false,
        ids: vec![None; depth + 1],
        global_excludes: None,
    }
}

#[test]
fn list_tree_matches_the_local_host() {
    let (host, project) = (remote(), Project::fixture());
    for (rel, show_ignored) in [("", false), ("", true), ("src", false)] {
        let request = list_request(&project, rel, show_ignored);
        let (remote, local) = (
            host.list_tree(&request),
            LocalTreeFs::new().list_tree(&request),
        );
        assert_eq!(remote.result.unwrap(), local.result.unwrap(), "{rel:?}");
        assert_eq!((remote.id, remote.lost), (local.id, local.lost), "{rel:?}");
    }
}

#[test]
fn list_tree_of_a_folder_that_is_gone_says_why_as_the_local_host_does() {
    let (host, project) = (remote(), Project::fixture());
    let request = list_request(&project, "gone", false);
    let (remote, local) = (
        host.list_tree(&request),
        LocalTreeFs::new().list_tree(&request),
    );
    assert_eq!(
        remote.result.unwrap_err().kind(),
        local.result.unwrap_err().kind()
    );
    assert_eq!((remote.id, remote.lost), (local.id, local.lost));
    assert!(remote.lost.is_some());
}

#[test]
fn exists_and_paste_name_match_the_local_host() {
    let (host, project) = (remote(), Project::fixture());
    let root = &project.root;
    let local = LocalTreeFs::new();
    for rel in ["a.txt", "dangling", "missing"] {
        let path = project.at(rel);
        assert_eq!(
            host.exists(root, &path).unwrap(),
            local.exists(root, &path).unwrap(),
            "{rel}"
        );
    }
    let planned = [OsString::from("a copy.txt")];
    let (from, dir) = (project.at("a.txt"), project.root.clone());
    assert_eq!(
        host.paste_name(root, &from, &dir, &planned).unwrap(),
        local.paste_name(root, &from, &dir, &planned).unwrap()
    );
    assert_eq!(host.restores_from_trash(), local.restores_from_trash());
    let outside = PathBuf::from("/");
    assert_eq!(
        kind(host.exists(root, &outside)),
        kind(local.exists(root, &outside))
    );
}

/// Runs `op` on a fixture through the remote host and on a twin of it
/// through the local one: their results, each fixture's root stripped from
/// it by `rel`, and their disks afterwards, are the same.
fn twins<T: PartialEq + std::fmt::Debug>(
    op: impl Fn(&dyn FsHost, &Project) -> io::Result<T>,
    rel: impl Fn(&Project, T) -> T,
) -> (Project, T) {
    let (host, remote_project, local_project) = (remote(), Project::fixture(), Project::fixture());
    let remote = op(&host, &remote_project).map(|t| rel(&remote_project, t));
    let local = op(&LocalTreeFs::new(), &local_project).map(|t| rel(&local_project, t));
    let remote = remote.unwrap();
    assert_eq!(remote, local.unwrap());
    assert_eq!(remote_project.snapshot(), local_project.snapshot());
    (remote_project, remote)
}

#[test]
fn create_recorded_matches_the_local_host() {
    let (project, created) = twins(
        |host, p| {
            let created =
                host.create_recorded(&p.root, &p.at("src"), "new/mod.rs", b"// new\n", false)?;
            Ok((created.path, created.first.path))
        },
        |p, (path, first)| (p.rel(&path), p.rel(&first)),
    );
    assert_eq!(created, ("src/new/mod.rs".into(), "src/new".into()));
    assert_eq!(
        std::fs::read_to_string(project.at("src/new/mod.rs")).unwrap(),
        "// new\n"
    );
}

#[test]
fn rename_and_move_into_match_the_local_host() {
    twins(
        |host, p| host.rename(&p.root, &p.at("a.txt"), "b.txt"),
        |p, to| p.rel(&to),
    );
    twins(
        |host, p| host.move_into(&p.root, &p.at("a.txt"), &p.at("src"), "a.md".as_ref()),
        |p, to| p.rel(&to),
    );
    // Neither replaces what's there.
    twins(
        |host, p| {
            Ok(host
                .rename(&p.root, &p.at("a.txt"), ".gitignore")
                .unwrap_err()
                .kind())
        },
        |_, kind| kind,
    );
}

#[test]
fn duplicate_and_copy_into_match_the_local_host() {
    let (project, placed) = twins(
        |host, p| {
            host.duplicate_recorded(&p.root, &p.at("src"))
                .map(|placed| placed.path)
        },
        |p, path| p.rel(&path),
    );
    assert_eq!(
        project
            .snapshot()
            .iter()
            .filter(|(rel, _)| rel == &placed)
            .count(),
        1
    );
    twins(
        |host, p| {
            let placed = host.copy_into_recorded(&p.root, &p.at("a.txt"), &p.at("src"))?;
            // What it made, as the host's disk has it.
            assert_eq!((placed.id.dev, placed.id.ino), dev_ino(&placed.path));
            Ok(placed.path)
        },
        |p, path| p.rel(&path),
    );
}

#[test]
fn truncate_matches_the_local_host() {
    let (project, ()) = twins(|host, p| host.truncate(&p.root, &p.at("a.txt")), |_, ()| ());
    assert_eq!(std::fs::read(project.at("a.txt")).unwrap(), b"");
}

#[test]
fn trash_and_restore_match_the_local_host() {
    let strip = |p: &Project, records: Vec<TrashRecord>| -> Vec<PathBuf> {
        records.iter().map(|r| p.rel(&r.path)).collect()
    };
    let (host, project) = (remote(), Project::fixture());
    let paths = [project.at("a.txt"), project.at("src")];
    let records = host.trash_recorded(&project.root, &paths).unwrap();
    assert_eq!(
        strip(&project, records.clone()),
        ["a.txt", "src"].map(PathBuf::from)
    );
    assert!(!project.at("a.txt").exists() && !project.at("src").exists());
    for record in &records {
        assert!(host.in_trash(record).unwrap());
        assert_eq!(
            host.in_trash(record).unwrap(),
            LocalTreeFs::new().in_trash(record).unwrap()
        );
    }
    for mut record in records {
        let path = host.restore(&project.root, &mut record).unwrap();
        assert_eq!(path, record.path);
        assert!(!host.in_trash(&record).unwrap());
    }
    assert_eq!(project.snapshot(), Project::fixture().snapshot());
}

#[test]
fn a_restore_onto_a_taken_path_fails_and_keeps_the_record() {
    let (host, project) = (remote(), Project::fixture());
    let [mut record] = <[TrashRecord; 1]>::try_from(
        host.trash_recorded(&project.root, &[project.at("a.txt")])
            .unwrap(),
    )
    .unwrap();
    project.write("a.txt", "a newcomer\n");
    let before = record.clone();
    let error = host.restore(&project.root, &mut record).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(record, before);
    assert_eq!(
        std::fs::read_to_string(project.at("a.txt")).unwrap(),
        "a newcomer\n"
    );
}

/// D § 11: "Its content appears in one round trip for ≤ 1 MiB (50.7 ms at
/// 25 ms one way)" with the 1.25 MiB window. A 1 MiB window takes two
/// (about 101 ms here), so the bound is one round trip and half of another
/// for slack, under the two; the best of three, so one scheduling stall on
/// a loaded machine doesn't count.
#[test]
fn a_one_mib_read_takes_one_round_trip() {
    let one_way = Duration::from_millis(25);
    let rtt = 2 * one_way;
    let slack = rtt / 2;
    let host = RemoteHost::new(Conn::new(Served::new(Hosts::local(), one_way).connection));
    let project = Project::new();
    let path = project.at("one.mib");
    std::fs::write(&path, vec![7u8; 1 << 20]).unwrap();
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let started = Instant::now();
        let (bytes, _) = host.read(&path).unwrap();
        best = best.min(started.elapsed());
        assert_eq!(bytes.len(), 1 << 20);
    }
    assert!(best >= rtt, "{best:?} is under a round trip");
    assert!(best < rtt + slack, "{best:?} is more than one round trip");
}

/// A host that holds up its reads and writes of `held` until the test lets
/// them go, saying when each has begun, and counts every call of each.
struct Held {
    inner: Arc<dyn FsHost>,
    held: Option<PathBuf>,
    begun: Mutex<mpsc::Sender<&'static str>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    reads: AtomicUsize,
    writes: AtomicUsize,
}

impl Held {
    fn new(held: Option<PathBuf>) -> (Arc<Held>, mpsc::Receiver<&'static str>) {
        let (begun, began) = mpsc::channel();
        let host = Arc::new(Held {
            inner: Arc::new(LocalTreeFs::new()),
            held,
            begun: Mutex::new(begun),
            gate: Arc::new((Mutex::new(false), Condvar::new())),
            reads: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
        });
        (host, began)
    }

    fn serve(self: &Arc<Self>) -> Served {
        let hosts = Hosts {
            fs: self.clone(),
            ..Hosts::local()
        };
        Served::new(hosts, Duration::ZERO)
    }

    fn hold(&self, path: &Path, what: &'static str) {
        if self.held.as_deref() != Some(path) {
            return;
        }
        let _ = self.begun.lock().unwrap().send(what);
        let (open, opened) = &*self.gate;
        let mut open = open.lock().unwrap();
        while !*open {
            open = opened.wait(open).unwrap();
        }
    }

    fn release(&self) {
        let (open, opened) = &*self.gate;
        *open.lock().unwrap() = true;
        opened.notify_all();
    }
}

impl FsHost for Held {
    fn read(&self, path: &Path) -> io::Result<(Vec<u8>, DiskStamp)> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.hold(path, "read");
        self.inner.read(path)
    }

    fn write_atomic(
        &self,
        path: &Path,
        bytes: &[u8],
        expect: WriteExpect,
    ) -> Result<DiskStamp, WriteError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.hold(path, "write");
        self.inner.write_atomic(path, bytes, expect)
    }

    fn stat(&self, path: &Path) -> io::Result<Option<fe_host::Stat>> {
        self.inner.stat(path)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<fe_host::DirEntry>> {
        self.inner.list_dir(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.inner.canonicalize(path)
    }
}

/// Cuts the link `cut` is a socket of, the connection `conn` holds, and
/// waits until its client has seen it go.
fn cut(cut: &UnixStream, conn: &Conn) {
    let connection = conn.current().unwrap();
    cut.shutdown(std::net::Shutdown::Both).unwrap();
    connection.mux().wait_lost();
}

#[test]
fn a_read_in_flight_when_the_link_drops_runs_again_on_the_next_connection() {
    let project = Project::fixture();
    let path = project.at("a.txt");
    let (first, began) = Held::new(Some(path.clone()));
    let Served {
        connection,
        cut: link,
    } = first.serve();
    let conn = Conn::new(connection);
    let reading = {
        let (host, path) = (RemoteHost::new(conn.clone()), path.clone());
        thread::spawn(move || host.read(&path))
    };
    assert_eq!(began.recv().unwrap(), "read");
    cut(&link, &conn);
    // It waits for the next connection: until there's one, nothing ends
    // it but a failure, which would show below.
    let (next, _) = Held::new(None);
    conn.reconnected(next.serve().connection);
    assert_eq!(
        reading.join().unwrap().unwrap(),
        LocalFs.read(&path).unwrap()
    );
    assert_eq!(next.reads.load(Ordering::SeqCst), 1);
    first.release();
}

#[test]
fn a_read_runs_again_only_once() {
    let project = Project::fixture();
    let path = project.at("a.txt");
    let (first, began) = Held::new(Some(path.clone()));
    let (second, began_again) = Held::new(Some(path.clone()));
    let Served {
        connection,
        cut: link,
    } = first.serve();
    let conn = Conn::new(connection);
    let reading = {
        let (host, path) = (RemoteHost::new(conn.clone()), path.clone());
        thread::spawn(move || host.read(&path))
    };
    assert_eq!(began.recv().unwrap(), "read");
    cut(&link, &conn);
    let Served {
        connection,
        cut: link_again,
    } = second.serve();
    conn.reconnected(connection);
    assert_eq!(began_again.recv().unwrap(), "read");
    cut(&link_again, &conn);
    let (third, _) = Held::new(None);
    conn.reconnected(third.serve().connection);
    let error = reading.join().unwrap().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
    assert!(
        matches!(LinkError::of(&error), Some(LinkError::Lost(_))),
        "{error:?}"
    );
    assert_eq!(third.reads.load(Ordering::SeqCst), 0);
    first.release();
    second.release();
}

#[test]
fn a_write_in_flight_when_the_link_drops_fails_and_isnt_sent_again() {
    let project = Project::fixture();
    let path = project.at("a.txt");
    let (first, began) = Held::new(Some(path.clone()));
    let Served {
        connection,
        cut: link,
    } = first.serve();
    let conn = Conn::new(connection);
    let writing = {
        let (host, path) = (RemoteHost::new(conn.clone()), path.clone());
        thread::spawn(move || host.write_atomic(&path, b"mine\n", WriteExpect::Overwrite))
    };
    assert_eq!(began.recv().unwrap(), "write");
    cut(&link, &conn);
    // It ends without waiting for a next connection.
    match writing.join().unwrap() {
        Err(WriteError::Io(error)) => {
            assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
            assert!(matches!(LinkError::of(&error), Some(LinkError::Lost(_))));
        }
        other => panic!("{other:?}"),
    }
    let (next, _) = Held::new(None);
    conn.reconnected(next.serve().connection);
    // A read on the next connection goes through, and no write came.
    RemoteHost::new(conn.clone()).stat(&path).unwrap();
    assert_eq!(next.writes.load(Ordering::SeqCst), 0);
    first.release();
}

#[test]
fn a_read_waiting_for_the_next_connection_fails_once_disconnected() {
    let project = Project::fixture();
    let path = project.at("a.txt");
    let (first, began) = Held::new(Some(path.clone()));
    let Served {
        connection,
        cut: link,
    } = first.serve();
    let conn = Conn::new(connection);
    let reading = {
        let (host, path) = (RemoteHost::new(conn.clone()), path.clone());
        thread::spawn(move || host.read(&path))
    };
    assert_eq!(began.recv().unwrap(), "read");
    cut(&link, &conn);
    conn.disconnected();
    let error = reading.join().unwrap().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotConnected);
    assert_eq!(LinkError::of(&error), Some(&LinkError::Disconnected));
    first.release();
}

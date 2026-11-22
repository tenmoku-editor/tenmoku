//! `RemoteHost` as `WatchHost` and `SearchHost` (D § 5, § 11, § 12): a
//! tenmoku-server session in this process, over the injected-latency
//! `Delayed` link. A watch delivers the host's changes in batches and
//! watches again on the next connection; Quick Open's list and a search
//! stream as the host walks; dropping either sends `Cancel`.

#![cfg(unix)]
#![allow(clippy::single_range_in_vec_init)]

use fe_host::{
    FileMatches, FilesSummary, FsChange, LineMatch, SearchFailed, SearchFilter, SearchHost,
    SearchOptions, SearchQuery, SearchStream, SearchSummary, Stream as HostStream, Subscription,
    WatchHost,
};
use fe_project::{LocalSearch, LocalWatch};
use fe_remote_client::{Conn, Connection, RemoteHost};
use fe_remote_proto::testing::{Latency, delayed};
use fe_remote_proto::{Config as MuxConfig, Location, Mux, PathBytes};
use std::io;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use tenmoku_server::{Config, Hosts};

const ONE_WAY: Duration = Duration::from_millis(2);

/// How long a test waits for the host's watcher to report a change: the
/// watcher's own latency, on a loaded machine. Never a sleep: the wait
/// ends when the change comes.
const WATCH_WAIT: Duration = Duration::from_secs(30);

/// A session served in this process for `hosts`, its client's link
/// delayed by [`ONE_WAY`] each way; shutting `cut` down drops the link.
fn serve(hosts: Hosts) -> (Connection, UnixStream) {
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
        Latency::one_way(ONE_WAY),
    );
    let config = MuxConfig {
        heartbeat: None,
        ..MuxConfig::default()
    };
    let location = Location {
        destination: "test".into(),
        path: PathBytes(b"/".to_vec()),
    };
    let (mux, welcome) = Mux::client(link, config, location, None).unwrap();
    (Connection::over(mux, welcome), cut)
}

fn remote_of(hosts: Hosts) -> RemoteHost {
    RemoteHost::new(Conn::new(serve(hosts).0))
}

fn remote() -> RemoteHost {
    remote_of(Hosts::local())
}

/// A project of `files`, by its canonical path.
fn project(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    for (rel, contents) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    (dir, root)
}

/// The watch's batches until one names `path`.
fn wait_for(watch: &Subscription<Vec<FsChange>>, path: &Path) -> Vec<Vec<FsChange>> {
    let mut batches = Vec::new();
    loop {
        let batch = watch.recv_timeout(WATCH_WAIT).expect("no change came");
        let found = batch.contains(&FsChange::Path(path.to_path_buf()));
        batches.push(batch);
        if found {
            return batches;
        }
    }
}

#[test]
fn a_watch_delivers_host_side_changes_as_fs_change_batches() {
    let (_dir, root) = project(&[("a.txt", "a")]);
    let watch = remote().watch(&root).unwrap();
    let new = root.join("new.txt");
    std::fs::write(&new, "made on the host").unwrap();
    let batches = wait_for(&watch, &new);
    assert!(batches.iter().all(|batch| !batch.is_empty()), "{batches:?}");
}

#[test]
fn a_watch_that_cant_start_fails_as_the_local_host_does() {
    let (_dir, root) = project(&[]);
    let missing = root.join("missing");
    let remote = remote().watch(&missing).unwrap_err();
    let local = LocalWatch.watch(&missing).unwrap_err();
    assert_eq!(remote.kind(), local.kind());
}

#[test]
fn a_watch_watches_again_on_the_next_connection_with_a_rescan() {
    let (_dir, root) = project(&[("a.txt", "a")]);
    let (connection, cut) = serve(Hosts::local());
    let conn = Conn::new(connection);
    let watch = RemoteHost::new(conn.clone()).watch(&root).unwrap();
    let first = conn.current().unwrap();
    cut.shutdown(std::net::Shutdown::Both).unwrap();
    first.mux().wait_lost();
    conn.reconnected(serve(Hosts::local()).0);
    // What changed while the link was down isn't known.
    let rescan = watch.recv_timeout(WATCH_WAIT).expect("no rescan came");
    assert_eq!(rescan, [FsChange::Rescan]);
    let after = root.join("after.txt");
    std::fs::write(&after, "made once watched again").unwrap();
    wait_for(&watch, &after);
}

/// A watch that reports nothing, and says when the server stops it.
struct QuietWatch {
    stopped: Mutex<mpsc::Sender<()>>,
}

/// A quiet watch's source: says when it's dropped, and holds the sender
/// the server's forwarder waits on.
struct Stops(
    mpsc::Sender<()>,
    #[allow(dead_code)] mpsc::Sender<Vec<FsChange>>,
);

impl Drop for Stops {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

impl WatchHost for QuietWatch {
    fn watch(&self, _root: &Path) -> io::Result<Subscription<Vec<FsChange>>> {
        let (changes, items) = mpsc::channel();
        let stopped = self.stopped.lock().unwrap().clone();
        Ok(Subscription::new(items, Stops(stopped, changes)))
    }
}

#[test]
fn dropping_a_watch_sends_cancel() {
    let (stopped, stops) = mpsc::channel();
    let host = remote_of(Hosts {
        watch: Arc::new(QuietWatch {
            stopped: Mutex::new(stopped),
        }),
        ..Hosts::local()
    });
    let watch = host.watch(Path::new("/")).unwrap();
    drop(watch);
    // The server stops its watch once the cancel comes, with the link
    // still up.
    stops.recv().unwrap();
    assert_eq!(host.conn().current().unwrap().mux().lost(), None);
}

/// A host whose file list is one batch of [`FIRST`], then, once the test
/// lets it go, the rest; and whose search finds one file, then nothing
/// until it is cancelled, which it says.
struct HeldWalk {
    gate: Arc<(Mutex<bool>, Condvar)>,
    cancelled: Mutex<mpsc::Sender<()>>,
}

const FIRST: usize = fe_remote_proto::FILES_BATCH;

impl HeldWalk {
    fn new() -> (Arc<HeldWalk>, mpsc::Receiver<()>) {
        let (cancelled, cancels) = mpsc::channel();
        let walk = HeldWalk {
            gate: Arc::new((Mutex::new(false), Condvar::new())),
            cancelled: Mutex::new(cancelled),
        };
        (Arc::new(walk), cancels)
    }

    fn release(&self) {
        let (open, opened) = &*self.gate;
        *open.lock().unwrap() = true;
        opened.notify_all();
    }

    fn paths(range: std::ops::Range<usize>) -> Vec<PathBuf> {
        range
            .map(|i| PathBuf::from(format!("f{i:04}.txt")))
            .collect()
    }
}

fn one_match(root: &Path) -> FileMatches {
    FileMatches {
        path: root.join("found.txt"),
        rel: "found.txt".into(),
        lines: vec![LineMatch {
            line_number: 1,
            line_offset: 0,
            ranges: vec![0..5],
            preview: "found".into(),
            preview_start: 0,
        }],
    }
}

impl SearchHost for HeldWalk {
    fn search(
        &self,
        root: &Path,
        _query: &SearchQuery,
        _options: &SearchOptions,
        _filter: &SearchFilter,
    ) -> SearchStream {
        let (tx, stream) = HostStream::channel(2);
        let (found, cancelled) = (one_match(root), self.cancelled.lock().unwrap().clone());
        thread::spawn(move || {
            tx.send(vec![found]);
            while !tx.is_cancelled() {
                thread::yield_now();
            }
            let _ = cancelled.send(());
            tx.end(Ok(SearchSummary {
                cancelled: true,
                ..SearchSummary::default()
            }));
        });
        stream
    }

    fn files(&self, _root: &Path, _filter: &SearchFilter) -> fe_host::FilesStream {
        let (tx, stream) = HostStream::channel(2);
        let gate = self.gate.clone();
        thread::spawn(move || {
            tx.send(HeldWalk::paths(0..FIRST));
            let (open, opened) = &*gate;
            let mut open = open.lock().unwrap();
            while !*open {
                open = opened.wait(open).unwrap();
            }
            tx.send(HeldWalk::paths(FIRST..FIRST + 100));
            tx.end(Ok(FilesSummary { complete: true }));
        });
        stream
    }
}

fn held_walk_host(walk: &Arc<HeldWalk>) -> RemoteHost {
    remote_of(Hosts {
        search: walk.clone(),
        ..Hosts::local()
    })
}

#[test]
fn quick_opens_list_streams_with_the_first_batch_before_the_walk_ends() {
    let (walk, _) = HeldWalk::new();
    let files = held_walk_host(&walk).files(Path::new("/project"), &SearchFilter::default());
    // The walk is held until the first batch is here.
    assert_eq!(files.recv().unwrap(), HeldWalk::paths(0..FIRST));
    walk.release();
    let rest = files.by_ref_flat();
    assert_eq!(rest, HeldWalk::paths(FIRST..FIRST + 100));
    assert_eq!(
        files.finish().unwrap().unwrap(),
        FilesSummary { complete: true }
    );
}

#[test]
fn dropping_a_search_stream_sends_cancel() {
    let (walk, cancels) = HeldWalk::new();
    let (_dir, root) = project(&[]);
    let host = held_walk_host(&walk);
    let found = host.search(
        &root,
        &SearchQuery::literal("found"),
        &SearchOptions::default(),
        &SearchFilter::default(),
    );
    assert_eq!(found.recv().unwrap(), [one_match(&root)]);
    drop(found);
    // The host's search is cancelled once the server has the cancel, with
    // the link still up: it wasn't the link's loss that stopped it.
    cancels.recv().unwrap();
    assert_eq!(host.conn().current().unwrap().mux().lost(), None);
}

#[test]
fn a_cancelled_file_list_ends_incomplete() {
    let (walk, _) = HeldWalk::new();
    let files = held_walk_host(&walk).files(Path::new("/project"), &SearchFilter::default());
    assert_eq!(files.recv().unwrap().len(), FIRST);
    assert_eq!(
        files.cancel().unwrap().unwrap(),
        FilesSummary { complete: false }
    );
    walk.release();
}

fn tree() -> (TempDir, PathBuf) {
    let mut files: Vec<(String, String)> = (0..40)
        .map(|i| {
            (
                format!("d{}/f{i}.txt", i % 4),
                format!("line\nneedle {i}\n"),
            )
        })
        .collect();
    files.push(("skip/x.txt".into(), "needle\n".into()));
    files.push((".gitignore".into(), "ignored/\n".into()));
    files.push(("ignored/y.txt".into(), "needle\n".into()));
    let files: Vec<(&str, &str)> = (files.iter())
        .map(|(rel, contents)| (rel.as_str(), contents.as_str()))
        .collect();
    project(&files)
}

fn all_files(stream: fe_host::FilesStream) -> (Vec<PathBuf>, FilesSummary) {
    let mut paths: Vec<PathBuf> = stream.by_ref_flat();
    paths.sort();
    (paths, stream.finish().unwrap().unwrap())
}

/// Collects a stream's batches, keeping the stream for its end.
trait ByRefFlat<T> {
    fn by_ref_flat(&self) -> Vec<T>;
}

impl<T, E> ByRefFlat<T> for HostStream<Vec<T>, E> {
    fn by_ref_flat(&self) -> Vec<T> {
        let mut all = Vec::new();
        while let Some(batch) = self.recv() {
            all.extend(batch);
        }
        all
    }
}

#[test]
fn the_file_list_matches_the_local_host() {
    let (_dir, root) = tree();
    for filter in [
        SearchFilter::default(),
        SearchFilter::globs(&[], &["skip/"]),
        SearchFilter::excluding("d1/f1.txt"),
    ] {
        assert_eq!(
            all_files(remote().files(&root, &filter)),
            all_files(LocalSearch.files(&root, &filter)),
            "{filter:?}"
        );
    }
}

#[test]
fn a_search_matches_the_local_host() {
    let (_dir, root) = tree();
    let options = SearchOptions {
        threads: 1,
        ..SearchOptions::default()
    };
    let filter = SearchFilter::globs(&["d*/"], &["d3/"]);
    let query = SearchQuery::literal("NEEDLE");
    let run = |stream: SearchStream| {
        let found = stream.by_ref_flat();
        (found, stream.finish().unwrap().unwrap())
    };
    let remote = run(remote().search(&root, &query, &options, &filter));
    let local = run(LocalSearch.search(&root, &query, &options, &filter));
    assert_eq!(remote, local);
    assert_eq!(remote.1.files_matched, 30);
}

#[test]
fn an_invalid_pattern_fails_as_the_local_host_does() {
    let (_dir, root) = tree();
    let query = SearchQuery {
        regex: true,
        ..SearchQuery::literal("(unclosed")
    };
    let options = SearchOptions::default();
    let filter = SearchFilter::default();
    let end = |stream: SearchStream| stream.finish().unwrap().unwrap_err();
    match (
        end(remote().search(&root, &query, &options, &filter)),
        end(LocalSearch.search(&root, &query, &options, &filter)),
    ) {
        (SearchFailed::InvalidPattern(remote), SearchFailed::InvalidPattern(local)) => {
            assert_eq!(remote, local);
        }
        other => panic!("{other:?}"),
    }
}

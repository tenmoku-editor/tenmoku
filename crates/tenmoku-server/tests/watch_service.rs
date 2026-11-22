//! Channel 2 (D § 2.2, § 5): a watch is a request that stays open,
//! streaming the host's `FsChange` batches, the wire type unchanged, until
//! a cancel ends it with `End(Cancelled)`.

#![cfg(unix)]

mod common;

use fe_host::{Subscription, WatchHost};
use fe_remote_proto::window::MIN_WINDOW;
use fe_remote_proto::{
    Config as MuxConfig, Event, FsChange, IoKind, Link, Mux, PathBytes, Status, Stream, WatchOp,
    chan,
};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use tenmoku_server::{Config, Hosts, serve};

/// A client of a server run in this process whose watches are `watch`'s.
fn connect(watch: Arc<dyn WatchHost>, client: MuxConfig) -> Mux {
    let config = Config {
        hosts: Arc::new(move || Hosts {
            watch: watch.clone(),
            ..Hosts::local()
        }),
        ..Config::default()
    };
    let (client_end, server_end) = Link::pipe().unwrap();
    thread::spawn(move || serve(server_end, config));
    common::handshake(client_end, client, None).unwrap().0
}

fn watch(client: &Mux, root: &Path) -> Stream {
    let op = WatchOp::Watch {
        root: PathBytes::from(root),
    };
    client.request(chan::WATCH, &op).unwrap()
}

/// The next batch, which must come before the watch ends.
fn batch(stream: &mut Stream) -> Vec<FsChange> {
    match stream.recv() {
        Some(Event::Data(bytes)) => stream.decode(&bytes).unwrap(),
        ev => panic!("{ev:?}"),
    }
}

/// What the watch ends with, after any batches still on their way.
fn end(stream: &mut Stream) -> Status {
    loop {
        match stream.recv() {
            Some(Event::Data(_)) => {}
            Some(Event::End(status)) => return status,
            None => panic!("the watch ended without an End"),
        }
    }
}

/// A watch host the test feeds by hand. Each batch is handed over only
/// once the server takes it, so the test knows what it holds.
#[derive(Default)]
struct FakeWatch {
    feeds: Mutex<Vec<Feed>>,
}

#[derive(Clone)]
struct Feed {
    changes: Arc<Mutex<Option<SyncSender<Vec<fe_host::FsChange>>>>>,
    stopped: Arc<Mutex<Receiver<()>>>,
}

/// What the subscription stops when it's dropped: the feed, as a real
/// watcher's sink goes with it.
struct Source {
    changes: Arc<Mutex<Option<SyncSender<Vec<fe_host::FsChange>>>>>,
    stopped: Sender<()>,
}

impl Drop for Source {
    fn drop(&mut self) {
        self.changes.lock().unwrap().take();
        let _ = self.stopped.send(());
    }
}

impl WatchHost for FakeWatch {
    fn watch(&self, _root: &Path) -> io::Result<Subscription<Vec<fe_host::FsChange>>> {
        let (tx, rx) = mpsc::sync_channel(0);
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let changes = Arc::new(Mutex::new(Some(tx)));
        self.feeds.lock().unwrap().push(Feed {
            changes: changes.clone(),
            stopped: Arc::new(Mutex::new(stopped_rx)),
        });
        let source = Source {
            changes,
            stopped: stopped_tx,
        };
        Ok(Subscription::new(rx, source))
    }
}

impl FakeWatch {
    fn feed(&self) -> Feed {
        self.feeds.lock().unwrap()[0].clone()
    }
}

impl Feed {
    /// Hands `batch` to the server, waiting until it takes it.
    fn send(&self, batch: Vec<fe_host::FsChange>) {
        let tx = self
            .changes
            .lock()
            .unwrap()
            .clone()
            .expect("the watch is on");
        tx.send(batch).unwrap();
    }

    /// Waits for the subscription to be dropped.
    fn wait_stopped(&self) {
        self.stopped.lock().unwrap().recv().unwrap();
    }
}

fn wire(path: &Path) -> FsChange {
    FsChange::Path(PathBytes::from(path))
}

#[test]
fn a_watch_streams_the_hosts_batches_path_and_rescan_alike() {
    let host = Arc::new(FakeWatch::default());
    let client = connect(host.clone(), common::client_config());
    let mut stream = watch(&client, Path::new("/project"));
    assert_eq!(batch(&mut stream), [], "the first batch says it's on");
    let feed = host.feed();

    let (a, b) = (
        PathBuf::from("/project/a.rs"),
        PathBuf::from("/project/b.rs"),
    );
    feed.send(vec![
        fe_host::FsChange::Path(a.clone()),
        fe_host::FsChange::Path(b.clone()),
    ]);
    assert_eq!(batch(&mut stream), [wire(&a), wire(&b)]);
    feed.send(vec![fe_host::FsChange::Rescan]);
    assert_eq!(batch(&mut stream), [FsChange::Rescan]);
}

#[test]
fn cancel_ends_a_watch_with_cancelled_and_stops_the_hosts() {
    let host = Arc::new(FakeWatch::default());
    let client = connect(host.clone(), common::client_config());
    let mut stream = watch(&client, Path::new("/project"));
    assert_eq!(batch(&mut stream), []);
    // Nothing changes: the cancel alone must end it.
    stream.cancel();
    assert_eq!(end(&mut stream), Status::Cancelled);
    host.feed().wait_stopped();
}

#[test]
fn changes_the_client_hasnt_taken_merge_until_a_rescan_covers_them() {
    let host = Arc::new(FakeWatch::default());
    // A small window, which the test doesn't read from: the server soon
    // has no credit left to send with.
    let client = connect(
        host.clone(),
        MuxConfig {
            window: MIN_WINDOW,
            ..common::client_config()
        },
    );
    let mut stream = watch(&client, Path::new("/project"));
    assert_eq!(batch(&mut stream), []);
    let feed = host.feed();
    let long = "n".repeat(200);
    // Before its credit runs out the server may take a window's worth
    // (under one batch here) and a merged batch of up to 10,000 changes,
    // however the threads run: what's fed after that must still pass
    // 10,000.
    let (batches, each) = (25, 1000);
    for b in 0..batches {
        let changes = (0..each)
            .map(|i| fe_host::FsChange::Path(format!("/project/{long}{b}-{i}").into()))
            .collect();
        feed.send(changes);
    }
    // Once this is taken, the server has merged the last batch.
    feed.send(Vec::new());

    let mut paths = 0;
    loop {
        let got = batch(&mut stream);
        if got.contains(&FsChange::Rescan) {
            assert_eq!(got.last(), Some(&FsChange::Rescan), "{got:?}");
            paths += got.len() - 1;
            break;
        }
        paths += got.len();
    }
    assert!(paths < batches * each, "{paths} paths came one by one");
    stream.cancel();
    assert_eq!(end(&mut stream), Status::Cancelled);
}

#[test]
fn a_watch_of_this_host_reports_a_file_made_once_it_is_on() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let client = connect(Arc::new(fe_project::LocalWatch), common::client_config());
    let mut stream = watch(&client, &root);
    assert_eq!(batch(&mut stream), []);

    let made = root.join("made.txt");
    std::fs::write(&made, "x").unwrap();
    loop {
        let got = batch(&mut stream);
        if got.contains(&wire(&made)) || got.contains(&FsChange::Rescan) {
            break;
        }
    }
    stream.cancel();
    assert_eq!(end(&mut stream), Status::Cancelled);
}

#[test]
fn a_watch_that_cant_start_ends_with_its_error() {
    let dir = tempfile::tempdir().unwrap();
    let client = connect(Arc::new(fe_project::LocalWatch), common::client_config());
    let mut stream = watch(&client, &dir.path().join("missing"));
    let Some(Event::End(Status::Error(e))) = stream.recv() else {
        panic!("the watch started");
    };
    assert_eq!(e.kind, IoKind::NotFound, "{}", e.message);
}

#[test]
fn a_watch_of_a_relative_root_is_refused() {
    let client = connect(Arc::new(FakeWatch::default()), common::client_config());
    let mut stream = watch(&client, Path::new("project"));
    let Some(Event::End(Status::Error(e))) = stream.recv() else {
        panic!("the watch started");
    };
    assert_eq!(e.kind, IoKind::InvalidInput);
}

//! Channel 1 (D § 2.2, § 5, § 7): each request is the `FsHost` call of
//! its name, answered as fe-project's `LocalTreeFs` answers it here.

#![cfg(unix)]

mod common;

use fe_host::FsHost as _;
use fe_project::LocalTreeFs;
use fe_remote_proto::window::MIN_WINDOW;
use fe_remote_proto::{
    CHUNK, ControlOp, DirEntry, DiskStamp, Event, FileKind, FsData, FsOp, IoKind, Link, ListTree,
    MAX_BODY, Mux, OpenTarget, PathBytes, Stat, Status, Stream, TrashRecord, WriteExpect, chan,
};
use std::ffi::OsString;
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use tempfile::TempDir;
use tenmoku_server::{Config, FS_WORKERS, Hosts, serve};

/// A client of a server run in this process, and a project folder.
struct Fixture {
    client: Mux,
    _dir: TempDir,
    /// `dir`'s canonical path: the project opens roots without following
    /// symlinks, and a temp dir may sit behind one (macOS's /var).
    root: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture::serving(Config::default())
    }

    fn serving(config: Config) -> Fixture {
        let (client_end, server_end) = Link::pipe().unwrap();
        thread::spawn(move || serve(server_end, config));
        let (client, _) = common::handshake(client_end, common::client_config(), None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        Fixture {
            client,
            _dir: dir,
            root,
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.root().join(rel)
    }

    fn call(&self, op: &FsOp) -> (Vec<FsData>, Status) {
        reply(self.client.request(chan::FS, op).unwrap())
    }

    /// A `WriteAtomic` of `bytes`, uploaded in chunks.
    fn write(&self, path: &Path, bytes: &[u8], expect: WriteExpect) -> (Vec<FsData>, Status) {
        let op = FsOp::WriteAtomic {
            path: wire(path),
            len: bytes.len() as u64,
            expect,
        };
        self.upload(&op, bytes)
    }

    /// `op`, followed by `bytes` in chunks.
    fn upload(&self, op: &FsOp, bytes: &[u8]) -> (Vec<FsData>, Status) {
        let stream = self.client.request(chan::FS, op).unwrap();
        for chunk in bytes.chunks(CHUNK) {
            stream.send(&FsData::Bytes(chunk.to_vec())).unwrap();
        }
        reply(stream)
    }

    /// Waits until the server has taken every request sent before this:
    /// the control channel's requests are taken in turn with them.
    fn sync(&self) {
        let open = ControlOp::Open(OpenTarget::Tcp {
            host: "localhost".into(),
            port: 1,
        });
        let mut stream = self.client.request(chan::CONTROL, &open).unwrap();
        assert!(matches!(stream.recv(), Some(Event::End(_))));
    }
}

fn reply(mut stream: Stream) -> (Vec<FsData>, Status) {
    let mut data = Vec::new();
    loop {
        match stream.recv() {
            Some(Event::Data(bytes)) => data.push(stream.decode(&bytes).unwrap()),
            Some(Event::End(status)) => return (data, status),
            None => panic!("the reply ended without an End"),
        }
    }
}

/// The one body a reply that ended well carried.
fn only(reply: (Vec<FsData>, Status)) -> FsData {
    let (data, status) = reply;
    assert_eq!(status, Status::Ok, "{data:?}");
    let [one] = <[FsData; 1]>::try_from(data).unwrap_or_else(|d| panic!("{d:?}"));
    one
}

fn error_kind(reply: (Vec<FsData>, Status)) -> IoKind {
    match reply.1 {
        Status::Error(e) => e.kind,
        status => panic!("{status:?}: {:?}", reply.0),
    }
}

fn wire(path: &Path) -> PathBytes {
    PathBytes::from(path)
}

fn wire_stamp(stamp: fe_host::DiskStamp) -> DiskStamp {
    DiskStamp {
        len: stamp.len,
        modified: stamp.modified.into(),
        dev: stamp.dev,
        ino: stamp.ino,
    }
}

fn stamp_of(path: &Path) -> DiskStamp {
    wire_stamp(fe_host::DiskStamp::of(path).unwrap().unwrap())
}

/// An entry's device and inode, as `lstat` reports them.
fn dev_ino(path: &Path) -> (u64, u64) {
    let meta = std::fs::symlink_metadata(path).unwrap();
    (meta.dev(), meta.ino())
}

#[test]
fn a_read_returns_the_bytes_and_the_hosts_stamp_in_64_kib_chunks() {
    let fx = Fixture::new();
    let path = fx.at("big.bin");
    let bytes: Vec<u8> = (0..2 * CHUNK + 1000).map(|i| (i % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();

    let (data, status) = fx.call(&FsOp::Read { path: wire(&path) });
    assert_eq!(status, Status::Ok);
    let FsData::File { stamp, len } = &data[0] else {
        panic!("{:?}", data[0]);
    };
    assert_eq!(*stamp, stamp_of(&path));
    assert_eq!(*len, bytes.len() as u64);
    let chunks: Vec<&[u8]> = (data[1..].iter())
        .map(|d| match d {
            FsData::Bytes(chunk) => &chunk[..],
            d => panic!("{d:?}"),
        })
        .collect();
    let sizes: Vec<usize> = chunks.iter().map(|c| c.len()).collect();
    assert_eq!(sizes, [CHUNK, CHUNK, 1000]);
    assert_eq!(chunks.concat(), bytes);
}

#[test]
fn a_read_of_a_missing_file_ends_with_not_found() {
    let fx = Fixture::new();
    let read = FsOp::Read {
        path: wire(&fx.at("missing")),
    };
    assert_eq!(error_kind(fx.call(&read)), IoKind::NotFound);
}

#[test]
fn a_plain_read_outside_the_project_is_served() {
    let fx = Fixture::new();
    let elsewhere = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(elsewhere.path())
        .unwrap()
        .join("notes.txt");
    std::fs::write(&path, "outside").unwrap();
    let (data, status) = fx.call(&FsOp::Read { path: wire(&path) });
    assert_eq!(status, Status::Ok);
    assert_eq!(data[1], FsData::Bytes(b"outside".to_vec()));
}

#[test]
fn a_relative_path_is_refused() {
    let fx = Fixture::new();
    let stat = FsOp::Stat {
        path: PathBytes::from("src/main.rs"),
    };
    assert_eq!(error_kind(fx.call(&stat)), IoKind::InvalidInput);
}

#[test]
fn write_atomic_overwrite_replaces_whatever_is_there() {
    let fx = Fixture::new();
    let path = fx.at("a.txt");
    std::fs::write(&path, "old").unwrap();
    let written = only(fx.write(&path, b"new", WriteExpect::Overwrite));
    assert_eq!(written, FsData::Written(stamp_of(&path)));
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn write_atomic_as_found_replaces_the_file_it_finds() {
    let fx = Fixture::new();
    let path = fx.at("a.txt");
    std::fs::write(&path, "old").unwrap();
    let written = only(fx.write(&path, b"new", WriteExpect::AsFound));
    assert_eq!(written, FsData::Written(stamp_of(&path)));
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn write_atomic_as_found_creates_a_file_at_a_path_found_empty() {
    let fx = Fixture::new();
    let path = fx.at("new.txt");
    let written = only(fx.write(&path, b"made", WriteExpect::AsFound));
    assert_eq!(written, FsData::Written(stamp_of(&path)));
    assert_eq!(std::fs::read(&path).unwrap(), b"made");
}

#[test]
fn write_atomic_absent_creates_a_new_file() {
    let fx = Fixture::new();
    let path = fx.at("new.txt");
    let written = only(fx.write(&path, b"made", WriteExpect::Absent));
    assert_eq!(written, FsData::Written(stamp_of(&path)));
    assert_eq!(std::fs::read(&path).unwrap(), b"made");
}

#[test]
fn write_atomic_absent_onto_an_existing_path_conflicts_and_leaves_the_file() {
    let fx = Fixture::new();
    let path = fx.at("taken.txt");
    std::fs::write(&path, "theirs").unwrap();
    let before = stamp_of(&path);
    let conflict = only(fx.write(&path, b"mine", WriteExpect::Absent));
    assert_eq!(
        conflict,
        FsData::Conflict {
            current: Some(before)
        }
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"theirs");
    assert_eq!(stamp_of(&path), before);
}

#[test]
fn write_atomic_matches_with_the_current_stamp_writes() {
    let fx = Fixture::new();
    let path = fx.at("a.txt");
    std::fs::write(&path, "old").unwrap();
    let written = only(fx.write(&path, b"new", WriteExpect::Matches(stamp_of(&path))));
    assert_eq!(written, FsData::Written(stamp_of(&path)));
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn write_atomic_matches_with_a_stale_stamp_conflicts_and_leaves_the_file() {
    let fx = Fixture::new();
    let path = fx.at("a.txt");
    std::fs::write(&path, "read by the client").unwrap();
    let stale = stamp_of(&path);
    std::fs::write(&path, "changed on the host").unwrap();
    let now = stamp_of(&path);
    assert_ne!(stale, now);

    let conflict = only(fx.write(&path, b"mine", WriteExpect::Matches(stale)));
    assert_eq!(conflict, FsData::Conflict { current: Some(now) });
    assert_eq!(std::fs::read(&path).unwrap(), b"changed on the host");
    assert_eq!(stamp_of(&path), now);
}

#[test]
fn an_upload_cancelled_before_its_last_byte_writes_nothing() {
    let fx = Fixture::new();
    let path = fx.at("a.txt");
    std::fs::write(&path, "old").unwrap();
    let op = FsOp::WriteAtomic {
        path: wire(&path),
        len: 10,
        expect: WriteExpect::Overwrite,
    };
    let stream = fx.client.request(chan::FS, &op).unwrap();
    stream.send(&FsData::Bytes(b"new".to_vec())).unwrap();
    stream.cancel();
    let (data, status) = reply(stream);
    assert_eq!(status, Status::Cancelled, "{data:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
}

#[test]
fn an_upload_past_its_length_is_refused_and_writes_nothing() {
    let fx = Fixture::new();
    let path = fx.at("a.txt");
    std::fs::write(&path, "old").unwrap();
    let op = FsOp::WriteAtomic {
        path: wire(&path),
        len: 2,
        expect: WriteExpect::Overwrite,
    };
    let stream = fx.client.request(chan::FS, &op).unwrap();
    stream.send(&FsData::Bytes(b"new".to_vec())).unwrap();
    assert_eq!(error_kind(reply(stream)), IoKind::InvalidData);
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
}

#[test]
fn stat_answers_as_the_local_host_does() {
    let fx = Fixture::new();
    std::fs::write(fx.at("file"), "x").unwrap();
    std::fs::create_dir(fx.at("dir")).unwrap();
    std::os::unix::fs::symlink(fx.at("file"), fx.at("link")).unwrap();
    for name in ["file", "dir", "link", "missing"] {
        let path = fx.at(name);
        let local = LocalTreeFs::new().stat(&path).unwrap().map(|s| Stat {
            kind: match s.kind {
                fe_host::FileKind::File => FileKind::File,
                fe_host::FileKind::Dir => FileKind::Dir,
                fe_host::FileKind::Symlink => FileKind::Symlink,
                fe_host::FileKind::Other => FileKind::Other,
            },
            stamp: wire_stamp(s.stamp),
        });
        let remote = only(fx.call(&FsOp::Stat { path: wire(&path) }));
        assert_eq!(remote, FsData::Stat(local), "{name}");
    }
}

#[test]
fn list_dir_answers_as_the_local_host_does() {
    let fx = Fixture::new();
    std::fs::write(fx.at("file"), "x").unwrap();
    std::fs::create_dir(fx.at("dir")).unwrap();
    std::os::unix::fs::symlink("dir", fx.at("link")).unwrap();
    let mut remote = list_dir(&fx, fx.root());
    remote.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        remote,
        [
            DirEntry {
                name: "dir".into(),
                kind: FileKind::Dir,
            },
            DirEntry {
                name: "file".into(),
                kind: FileKind::File,
            },
            DirEntry {
                name: "link".into(),
                kind: FileKind::Symlink,
            },
        ]
    );
    assert_eq!(
        LocalTreeFs::new().list_dir(fx.root()).unwrap().len(),
        remote.len()
    );
}

fn list_dir(fx: &Fixture, path: &Path) -> Vec<DirEntry> {
    let (data, status) = fx.call(&FsOp::ListDir { path: wire(path) });
    assert_eq!(status, Status::Ok);
    (data.into_iter())
        .flat_map(|d| match d {
            FsData::Entries(entries) => entries,
            d => panic!("{d:?}"),
        })
        .collect()
}

#[test]
fn a_big_listing_comes_in_frames_of_at_most_a_chunk() {
    let fx = Fixture::new();
    let long = "n".repeat(200);
    for i in 0..1000 {
        std::fs::write(fx.at(&format!("{long}{i:04}")), "").unwrap();
    }
    let (data, status) = fx.call(&FsOp::ListDir {
        path: wire(fx.root()),
    });
    assert_eq!(status, Status::Ok);
    assert!(data.len() > 1, "{} frames", data.len());
    for body in &data {
        assert!(fe_remote_proto::encoded_len(body).unwrap() <= CHUNK);
    }
    assert_eq!(list_dir(&fx, fx.root()).len(), 1000);
}

#[test]
fn canonicalize_answers_as_the_local_host_does() {
    let fx = Fixture::new();
    std::fs::create_dir(fx.at("real")).unwrap();
    std::os::unix::fs::symlink(fx.at("real"), fx.at("alias")).unwrap();
    let path = fx.at("alias/../alias");
    let local = LocalTreeFs::new().canonicalize(&path).unwrap();
    let remote = only(fx.call(&FsOp::Canonicalize { path: wire(&path) }));
    assert_eq!(remote, FsData::Path(wire(&local)));
}

fn list_tree(root: &Path, rel: &str, show_ignored: bool) -> ListTree {
    let names = Path::new(rel).components().count();
    ListTree {
        root: wire(root),
        rel: PathBytes::from(rel),
        show_ignored,
        parent_ignored: false,
        ids: vec![None; names + 1],
    }
}

#[test]
fn list_tree_answers_as_the_local_host_does() {
    let fx = Fixture::new();
    std::fs::create_dir_all(fx.at("src/nested")).unwrap();
    std::fs::write(fx.at("src/main.rs"), "").unwrap();
    std::fs::write(fx.at("src/debug.log"), "").unwrap();
    std::fs::write(fx.at(".gitignore"), "*.log\n").unwrap();
    std::fs::create_dir(fx.at(".git")).unwrap();
    std::os::unix::fs::symlink("main.rs", fx.at("src/link")).unwrap();

    let list = list_tree(fx.root(), "src", true);
    let local = LocalTreeFs::new().list_tree(&fe_host::ListRequest {
        root: fx.root().to_path_buf(),
        rel: "src".into(),
        show_ignored: true,
        parent_ignored: false,
        ids: vec![None, None],
        global_excludes: None,
    });
    let (data, status) = fx.call(&FsOp::ListTree(list));
    assert_eq!(status, Status::Ok);
    let (last, batches) = data.split_last().unwrap();
    let remote: Vec<fe_remote_proto::Entry> = (batches.iter())
        .flat_map(|d| match d {
            FsData::Tree(entries) => entries.clone(),
            d => panic!("{d:?}"),
        })
        .collect();
    let expected: Vec<fe_remote_proto::Entry> = (local.result.unwrap().iter())
        .map(|e| fe_remote_proto::Entry {
            name: e.name.as_os_str().into(),
            kind: match e.kind {
                fe_host::EntryKind::File => fe_remote_proto::EntryKind::File,
                fe_host::EntryKind::Dir => fe_remote_proto::EntryKind::Dir,
                fe_host::EntryKind::Symlink => fe_remote_proto::EntryKind::Symlink,
            },
            ignored: e.ignored,
            id: e.id,
        })
        .collect();
    assert_eq!(remote, expected);
    assert!(remote.iter().any(|e| e.ignored), "{remote:?}");
    assert_eq!(*last, FsData::Listed { id: local.id });
    assert_eq!(local.id, Some(dev_ino(&fx.at("src"))));
}

#[test]
fn list_tree_of_a_folder_that_is_gone_says_so_before_its_error() {
    let fx = Fixture::new();
    let (data, status) = fx.call(&FsOp::ListTree(list_tree(fx.root(), "gone", false)));
    let local = LocalTreeFs::new().list_tree(&fe_host::ListRequest {
        root: fx.root().to_path_buf(),
        rel: "gone".into(),
        show_ignored: false,
        parent_ignored: false,
        ids: vec![None, None],
        global_excludes: None,
    });
    let local_error = local.result.unwrap_err();
    assert_eq!(
        data,
        [FsData::Unlisted {
            lost: local.lost.map(|lost| match lost {
                fe_host::ListingLost::Itself => fe_remote_proto::ListingLost::Itself,
                fe_host::ListingLost::Above => fe_remote_proto::ListingLost::Above,
                fe_host::ListingLost::Root => fe_remote_proto::ListingLost::Root,
            }),
        }]
    );
    let Status::Error(e) = status else {
        panic!("{status:?}");
    };
    assert_eq!(e.kind, local_error.kind().into());
}

#[test]
fn a_tree_operation_leaving_the_root_is_refused() {
    let fx = Fixture::new();
    let list = list_tree(fx.root(), "../elsewhere", false);
    assert_eq!(
        error_kind(fx.call(&FsOp::ListTree(list))),
        IoKind::InvalidInput
    );
    let rename = FsOp::Rename {
        root: wire(fx.root()),
        from: wire(&fx.at("../outside")),
        new_name: "x".into(),
    };
    assert_eq!(error_kind(fx.call(&rename)), IoKind::InvalidInput);
}

#[test]
fn create_recorded_says_what_it_made() {
    let fx = Fixture::new();
    let op = FsOp::CreateRecorded {
        root: wire(fx.root()),
        dir: wire(fx.root()),
        name: "a/b.txt".into(),
        len: 5,
        folder: false,
    };
    let FsData::Created(created) = only(fx.upload(&op, b"hello")) else {
        panic!();
    };
    assert_eq!(created.path, wire(&fx.at("a/b.txt")));
    assert_eq!(created.first.path, wire(&fx.at("a")));
    let (dev, ino) = dev_ino(&fx.at("a"));
    assert_eq!((created.first.id.dev, created.first.id.ino), (dev, ino));
    assert_eq!(std::fs::read(fx.at("a/b.txt")).unwrap(), b"hello");
}

#[test]
fn rename_returns_the_new_path_and_never_replaces() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    std::fs::write(fx.at("b.txt"), "b").unwrap();
    let rename = |to: &str| FsOp::Rename {
        root: wire(fx.root()),
        from: wire(&fx.at("a.txt")),
        new_name: to.into(),
    };
    assert_eq!(error_kind(fx.call(&rename("b.txt"))), IoKind::AlreadyExists);
    assert_eq!(
        only(fx.call(&rename("c.txt"))),
        FsData::Path(wire(&fx.at("c.txt")))
    );
    assert_eq!(std::fs::read(fx.at("c.txt")).unwrap(), b"a");
    assert_eq!(std::fs::read(fx.at("b.txt")).unwrap(), b"b");
}

#[test]
fn duplicate_recorded_places_a_copy_beside_the_entry() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    let op = FsOp::DuplicateRecorded {
        root: wire(fx.root()),
        path: wire(&fx.at("a.txt")),
    };
    let FsData::Placed(placed) = only(fx.call(&op)) else {
        panic!();
    };
    let copy = placed.path.to_path_buf();
    assert_eq!(copy.parent(), Some(fx.root()));
    assert_ne!(copy, fx.at("a.txt"));
    assert_eq!(std::fs::read(&copy).unwrap(), b"a");
    assert_eq!((placed.id.dev, placed.id.ino), dev_ino(&copy));
}

#[test]
fn copy_into_recorded_places_a_copy_in_the_folder() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    std::fs::create_dir(fx.at("dir")).unwrap();
    let op = FsOp::CopyIntoRecorded {
        root: wire(fx.root()),
        path: wire(&fx.at("a.txt")),
        dir: wire(&fx.at("dir")),
    };
    let FsData::Placed(placed) = only(fx.call(&op)) else {
        panic!();
    };
    assert_eq!(placed.path, wire(&fx.at("dir/a.txt")));
    assert_eq!((placed.id.dev, placed.id.ino), dev_ino(&fx.at("dir/a.txt")));
}

#[test]
fn paste_name_passes_over_the_planned_names() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    let planned = vec![PathBytes::from("a copy.txt")];
    let local = LocalTreeFs::new()
        .paste_name(
            fx.root(),
            &fx.at("a.txt"),
            fx.root(),
            &["a copy.txt".into()],
        )
        .unwrap();
    let op = FsOp::PasteName {
        root: wire(fx.root()),
        path: wire(&fx.at("a.txt")),
        dir: wire(fx.root()),
        planned,
    };
    assert_eq!(only(fx.call(&op)), FsData::Name(local.as_os_str().into()));
    assert_ne!(local, "a copy.txt");
}

#[test]
fn move_into_moves_the_entry_and_never_replaces() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    std::fs::create_dir(fx.at("dir")).unwrap();
    let op = FsOp::MoveInto {
        root: wire(fx.root()),
        from: wire(&fx.at("a.txt")),
        dir: wire(&fx.at("dir")),
        name: "moved.txt".into(),
    };
    assert_eq!(
        only(fx.call(&op)),
        FsData::Path(wire(&fx.at("dir/moved.txt")))
    );
    assert!(!fx.at("a.txt").exists());
    std::fs::write(fx.at("b.txt"), "b").unwrap();
    let again = FsOp::MoveInto {
        root: wire(fx.root()),
        from: wire(&fx.at("b.txt")),
        dir: wire(&fx.at("dir")),
        name: "moved.txt".into(),
    };
    assert_eq!(error_kind(fx.call(&again)), IoKind::AlreadyExists);
    assert_eq!(std::fs::read(fx.at("dir/moved.txt")).unwrap(), b"a");
}

#[test]
fn restores_from_trash_answers_as_the_local_host_does() {
    let fx = Fixture::new();
    let local = LocalTreeFs::new().restores_from_trash();
    assert_eq!(only(fx.call(&FsOp::RestoresFromTrash)), FsData::Bool(local));
}

/// Trashes `rel`, through the server, into the tests' own trash.
fn trash(fx: &Fixture, rel: &str) -> TrashRecord {
    let op = FsOp::TrashRecorded {
        root: wire(fx.root()),
        paths: vec![wire(&fx.at(rel))],
    };
    let FsData::Trashed(records) = only(fx.call(&op)) else {
        panic!();
    };
    let [record] = <[TrashRecord; 1]>::try_from(records).unwrap();
    record
}

#[test]
fn trash_recorded_records_each_entry_it_moves() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    let id = dev_ino(&fx.at("a.txt"));
    let record = trash(&fx, "a.txt");
    assert!(!fx.at("a.txt").exists());
    assert_eq!(record.path, wire(&fx.at("a.txt")));
    assert_eq!((record.id.dev, record.id.ino), id);
    let in_trash = FsOp::InTrash {
        trashed: record.clone(),
    };
    assert_eq!(only(fx.call(&in_trash)), FsData::Bool(true));
}

#[test]
fn restore_puts_the_entry_back_and_sends_its_record_first() {
    let fx = Fixture::new();
    if !LocalTreeFs::new().restores_from_trash() {
        return;
    }
    std::fs::write(fx.at("a.txt"), "a").unwrap();
    let record = trash(&fx, "a.txt");
    let op = FsOp::Restore {
        root: wire(fx.root()),
        trashed: record.clone(),
    };
    let (data, status) = fx.call(&op);
    assert_eq!(status, Status::Ok, "{data:?}");
    let [FsData::Record(after), FsData::Path(path)] = &data[..] else {
        panic!("{data:?}");
    };
    assert_eq!(*path, wire(&fx.at("a.txt")));
    assert_eq!(after.path, record.path);
    assert_eq!(after.id, record.id);
    assert_eq!(std::fs::read(fx.at("a.txt")).unwrap(), b"a");
    let in_trash = FsOp::InTrash {
        trashed: after.clone(),
    };
    assert_eq!(only(fx.call(&in_trash)), FsData::Bool(false));
}

#[test]
fn exists_counts_a_dangling_symlink() {
    let fx = Fixture::new();
    std::os::unix::fs::symlink("nowhere", fx.at("dangling")).unwrap();
    let exists = |rel: &str| FsOp::Exists {
        root: wire(fx.root()),
        path: wire(&fx.at(rel)),
    };
    assert_eq!(only(fx.call(&exists("dangling"))), FsData::Bool(true));
    assert_eq!(only(fx.call(&exists("missing"))), FsData::Bool(false));
}

#[test]
fn truncate_empties_the_file_in_place() {
    let fx = Fixture::new();
    std::fs::write(fx.at("a.txt"), "contents").unwrap();
    let id = dev_ino(&fx.at("a.txt"));
    let op = FsOp::Truncate {
        root: wire(fx.root()),
        path: wire(&fx.at("a.txt")),
    };
    let (data, status) = fx.call(&op);
    assert_eq!((data, status), (Vec::new(), Status::Ok));
    assert_eq!(std::fs::read(fx.at("a.txt")).unwrap(), b"");
    assert_eq!(dev_ino(&fx.at("a.txt")), id);
}

#[test]
fn create_recorded_takes_contents_larger_than_a_request() {
    let fx = Fixture::new();
    let contents: Vec<u8> = (0..MAX_BODY + MAX_BODY / 2)
        .map(|i| (i % 253) as u8)
        .collect();
    let op = FsOp::CreateRecorded {
        root: wire(fx.root()),
        dir: wire(fx.root()),
        name: "big.bin".into(),
        len: contents.len() as u64,
        folder: false,
    };
    let FsData::Created(created) = only(fx.upload(&op, &contents)) else {
        panic!();
    };
    assert_eq!(created.path, wire(&fx.at("big.bin")));
    assert_eq!(std::fs::read(fx.at("big.bin")).unwrap(), contents);
}

#[test]
fn more_uploads_than_workers_at_once_all_complete() {
    // A window of four chunks, and four writes more than there are
    // workers: their first chunks, unread, would hold all of it.
    let mut config = Config::default();
    config.mux.window = MIN_WINDOW;
    let fx = Fixture::serving(config);
    let count = FS_WORKERS + (MIN_WINDOW as usize).div_ceil(CHUNK);
    let streams: Vec<(PathBuf, Stream)> = (0..count)
        .map(|i| {
            let path = fx.at(&format!("f{i}"));
            let op = FsOp::WriteAtomic {
                path: wire(&path),
                len: 2 * CHUNK as u64,
                expect: WriteExpect::Overwrite,
            };
            (path, fx.client.request(chan::FS, &op).unwrap())
        })
        .collect();
    // Every write's first half, then every write's second: no write is
    // whole until the last of the first halves is in.
    for half in [0u8, 1] {
        for (_, stream) in &streams {
            stream.send(&FsData::Bytes(vec![half; CHUNK])).unwrap();
        }
    }
    for (path, stream) in streams {
        let written = only(reply(stream));
        assert_eq!(written, FsData::Written(stamp_of(&path)));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 2 * CHUNK);
        assert!(bytes[..CHUNK].iter().all(|&b| b == 0) && bytes[CHUNK..].iter().all(|&b| b == 1));
    }
}

/// This machine's files, whose reads wait at a gate the test opens: they
/// hold the fs workers.
struct HeldReads {
    entered: Mutex<mpsc::Sender<()>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

impl fe_host::FsHost for HeldReads {
    fn read(&self, path: &Path) -> io::Result<(Vec<u8>, fe_host::DiskStamp)> {
        self.entered.lock().unwrap().send(()).unwrap();
        let (open, opened) = &*self.gate;
        let mut open = open.lock().unwrap();
        while !*open {
            open = opened.wait(open).unwrap();
        }
        LocalTreeFs::new().read(path)
    }

    fn write_atomic(
        &self,
        path: &Path,
        bytes: &[u8],
        expect: fe_host::WriteExpect,
    ) -> Result<fe_host::DiskStamp, fe_host::WriteError> {
        LocalTreeFs::new().write_atomic(path, bytes, expect)
    }

    fn stat(&self, path: &Path) -> io::Result<Option<fe_host::Stat>> {
        LocalTreeFs::new().stat(path)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<fe_host::DirEntry>> {
        LocalTreeFs::new().list_dir(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        LocalTreeFs::new().canonicalize(path)
    }

    fn rename(&self, root: &Path, from: &Path, new_name: &str) -> io::Result<PathBuf> {
        LocalTreeFs::new().rename(root, from, new_name)
    }

    fn truncate(&self, root: &Path, path: &Path) -> io::Result<()> {
        LocalTreeFs::new().truncate(root, path)
    }

    fn paste_name(
        &self,
        root: &Path,
        path: &Path,
        dir: &Path,
        planned: &[OsString],
    ) -> io::Result<OsString> {
        LocalTreeFs::new().paste_name(root, path, dir, planned)
    }
}

#[test]
fn mutations_cancelled_while_they_wait_for_a_worker_change_nothing() {
    let (entered_tx, entered) = mpsc::channel();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let host = Arc::new(HeldReads {
        entered: Mutex::new(entered_tx),
        gate: gate.clone(),
    });
    let config = Config {
        hosts: Arc::new(move || Hosts {
            fs: host.clone(),
            ..Hosts::local()
        }),
        ..Config::default()
    };
    let fx = Fixture::serving(config);
    std::fs::write(fx.at("held"), "held").unwrap();
    std::fs::write(fx.at("a.txt"), "contents").unwrap();
    let reads: Vec<Stream> = (0..FS_WORKERS)
        .map(|_| {
            let read = FsOp::Read {
                path: wire(&fx.at("held")),
            };
            fx.client.request(chan::FS, &read).unwrap()
        })
        .collect();
    for _ in 0..FS_WORKERS {
        entered.recv().unwrap();
    }

    let truncate = FsOp::Truncate {
        root: wire(fx.root()),
        path: wire(&fx.at("a.txt")),
    };
    let rename = FsOp::Rename {
        root: wire(fx.root()),
        from: wire(&fx.at("a.txt")),
        new_name: "b.txt".into(),
    };
    let queued: Vec<Stream> = [truncate, rename]
        .iter()
        .map(|op| fx.client.request(chan::FS, op).unwrap())
        .collect();
    // They wait behind the held reads; cancel them there.
    fx.sync();
    for stream in &queued {
        stream.cancel();
    }
    fx.sync();
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();

    for stream in queued {
        assert_eq!(reply(stream).1, Status::Cancelled);
    }
    assert_eq!(std::fs::read(fx.at("a.txt")).unwrap(), b"contents");
    assert!(!fx.at("b.txt").exists());
    for read in reads {
        assert_eq!(reply(read).1, Status::Ok);
    }
}

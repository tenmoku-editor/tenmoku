//! Channel 1's bodies: `FsHost` (§ 5) and write preconditions (§ 7).

use crate::PathBytes;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// A `Request` body on channel 1: one of fe-host's `FsHost` calls, which
/// the host answers as its local impl does. Paths are absolute on the
/// host, but for the tree operations' paths, which may be relative to
/// their `root`. A call that fails ends with `Status::Error`, and one
/// cancelled ends with `Status::Cancelled`: before the host was called, if
/// the cancel reached it first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsOp {
    /// Replies [`FsData::File`], then the contents as [`FsData::Bytes`]
    /// chunks of at most [`crate::CHUNK`].
    Read { path: PathBytes },
    /// Replaces the file at `path` if `expect` holds. The client follows
    /// the request with exactly `len` bytes as [`FsData::Bytes`] chunks on
    /// the same channel and id, counted against the server's credit. Once
    /// they are all in, the server commits and replies [`FsData::Written`]
    /// or [`FsData::Conflict`].
    WriteAtomic {
        path: PathBytes,
        len: u64,
        expect: WriteExpect,
    },
    /// What is at `path`, symlinks followed: replies [`FsData::Stat`],
    /// `None` when nothing is.
    Stat { path: PathBytes },
    /// The directory's entries, in no order: replies [`FsData::Entries`]
    /// in batches.
    ListDir { path: PathBytes },
    /// Replies [`FsData::Path`] with the canonical path.
    Canonicalize { path: PathBytes },

    // The project tree's operations and its undo's (fe-host's `FsHost`
    // has the rules): every path is reached from `root` without following
    // a symlink, and one that leaves it is refused.
    /// Replies the entries in display order as [`FsData::Tree`] batches,
    /// then [`FsData::Listed`]; or, on failure, [`FsData::Unlisted`] before
    /// the error.
    ListTree(ListTree),
    /// Replies [`FsData::Created`]. The new file's contents follow the
    /// request as `WriteAtomic`'s do: exactly `len` bytes as
    /// [`FsData::Bytes`] chunks, which the server takes before it creates
    /// anything.
    CreateRecorded {
        root: PathBytes,
        dir: PathBytes,
        name: String,
        len: u64,
        folder: bool,
    },
    /// Replies [`FsData::Path`].
    Rename {
        root: PathBytes,
        from: PathBytes,
        new_name: String,
    },
    /// Replies [`FsData::Placed`].
    DuplicateRecorded { root: PathBytes, path: PathBytes },
    /// Replies [`FsData::Placed`].
    CopyIntoRecorded {
        root: PathBytes,
        path: PathBytes,
        dir: PathBytes,
    },
    /// Replies [`FsData::Name`].
    PasteName {
        root: PathBytes,
        path: PathBytes,
        dir: PathBytes,
        planned: Vec<PathBytes>,
    },
    /// Replies [`FsData::Path`].
    MoveInto {
        root: PathBytes,
        from: PathBytes,
        dir: PathBytes,
        name: PathBytes,
    },
    /// Replies [`FsData::Trashed`] batches, in the order of `paths`.
    TrashRecorded {
        root: PathBytes,
        paths: Vec<PathBytes>,
    },
    /// Replies [`FsData::Bool`].
    RestoresFromTrash,
    /// Replies [`FsData::Record`], the record as it is now, on failure
    /// too, then [`FsData::Path`] on success.
    Restore {
        root: PathBytes,
        trashed: TrashRecord,
    },
    /// Replies [`FsData::Bool`].
    InTrash { trashed: TrashRecord },
    /// Replies [`FsData::Bool`].
    Exists { root: PathBytes, path: PathBytes },
    /// Replies nothing: its `End(Ok)` says it's done.
    Truncate { root: PathBytes, path: PathBytes },
}

/// fe-host's `ListRequest`, less the global excludes, which are the
/// client's own and never cross the wire: the host lists with its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTree {
    pub root: PathBytes,
    /// Relative to `root`: plain names only.
    pub rel: PathBytes,
    pub show_ignored: bool,
    pub parent_ignored: bool,
    /// The device and inode the tree has for the root and each directory
    /// down to `rel`, `None` where it doesn't know.
    pub ids: Vec<Option<(u64, u64)>>,
}

/// A `Data` body on channel 1, either way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsData {
    /// A chunk of a file's contents, at most [`crate::CHUNK`].
    Bytes(#[serde(with = "serde_bytes")] Vec<u8>),
    /// The file a `Read` opened: its stamp, and how many bytes follow.
    File {
        stamp: DiskStamp,
        len: u64,
    },
    /// A `WriteAtomic` committed; the file's new stamp.
    Written(DiskStamp),
    /// A `WriteAtomic`'s `expect` didn't hold, so nothing was written.
    /// `current` is what is at the path now, if anything.
    Conflict {
        current: Option<DiskStamp>,
    },
    Stat(Option<Stat>),
    Entries(Vec<DirEntry>),
    Path(PathBytes),
    Tree(Vec<Entry>),
    /// A `ListTree` read its directory, whose device and inode these are.
    Listed {
        id: Option<(u64, u64)>,
    },
    /// A `ListTree` failed; `lost` says whether the path no longer led to
    /// the directory the listing was for.
    Unlisted {
        lost: Option<ListingLost>,
    },
    Created(Created),
    Placed(Placed),
    Name(PathBytes),
    Trashed(Vec<TrashRecord>),
    Record(TrashRecord),
    Bool(bool),
}

/// What a write expects to replace (§ 7). fe-editor's `SaveJob` maps onto
/// it one to one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteExpect {
    /// Overwrite was chosen: replace whatever is there.
    Overwrite,
    /// The buffer doesn't know the file's state. What the server finds when
    /// the write starts must still be there at the commit, and a path found
    /// empty is committed without clobbering.
    AsFound,
    /// Nothing, not even a symlink, may be at the path.
    Absent,
    /// The file must still be the one with this stamp.
    Matches(DiskStamp),
}

/// fe-buffer's `DiskStamp`, computed on the host, so `dev` and `ino` are
/// the host's. The client only compares stamps for equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiskStamp {
    pub len: u64,
    pub modified: Time,
    pub dev: u64,
    pub ino: u64,
}

/// A `SystemTime` as seconds and nanoseconds from the unix epoch; `secs` is
/// negative before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Time {
    pub secs: i64,
    /// Below 1,000,000,000.
    pub nanos: u32,
}

impl From<SystemTime> for Time {
    fn from(t: SystemTime) -> Time {
        match t.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => Time {
                secs: d.as_secs() as i64,
                nanos: d.subsec_nanos(),
            },
            Err(e) => {
                let d = e.duration();
                let (secs, nanos) = (d.as_secs() as i64, d.subsec_nanos());
                if nanos == 0 {
                    Time { secs: -secs, nanos }
                } else {
                    Time {
                        secs: -secs - 1,
                        nanos: 1_000_000_000 - nanos,
                    }
                }
            }
        }
    }
}

impl Time {
    /// The `SystemTime`, or `None` where this machine can't hold it.
    pub fn to_system_time(self) -> Option<SystemTime> {
        let nanos = Duration::from_nanos(u64::from(self.nanos));
        if self.secs >= 0 {
            SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(self.secs as u64))?
                .checked_add(nanos)
        } else {
            SystemTime::UNIX_EPOCH
                .checked_sub(Duration::from_secs(self.secs.unsigned_abs()))?
                .checked_add(nanos)
        }
    }
}

/// What `Stat` found at a path, symlinks followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stat {
    pub kind: FileKind,
    pub stamp: DiskStamp,
}

/// What is at a path, as fe-host's `FileKind` has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    /// A FIFO, socket, device and the like.
    Other,
}

/// One entry of a `ListDir`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: PathBytes,
    /// The entry itself, a symlink not followed.
    pub kind: FileKind,
}

/// What a tree entry is; a symlink is never followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

/// One entry of a `ListTree`, as fe-project's listing has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: PathBytes,
    pub kind: EntryKind,
    /// An ignore file matches it. Only ever set when `show_ignored` is.
    pub ignored: bool,
    /// A directory's device and inode, from `stat`; `None` for anything
    /// else.
    pub id: Option<(u64, u64)>,
}

/// Why a path no longer leads to the directory a listing was for, as
/// fe-host's `ListingLost` has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListingLost {
    Itself,
    Above,
    Root,
}

/// fe-host's `EntryId`: what an entry is, compared as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntryId {
    pub dev: u64,
    pub ino: u64,
    /// The birth time as seconds and nanoseconds since the Unix epoch, where
    /// the host's file system reports one.
    pub born: Option<(i64, u32)>,
}

/// An entry an operation made: its path, absolute under the root, and what
/// it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub path: PathBytes,
    pub id: EntryId,
}

/// What a recorded create made: the entry's path, and the first entry it
/// made, the top of what undoing it removes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub path: PathBytes,
    pub first: Placed,
}

/// An entry a recorded trash moved to the host's Trash, as fe-host's
/// `TrashRecord` has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashRecord {
    pub path: PathBytes,
    pub at: Option<PathBytes>,
    pub id: EntryId,
}

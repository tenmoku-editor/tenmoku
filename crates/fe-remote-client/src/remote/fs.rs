//! [`FsHost`] over channel 1: each call is the [`FsOp`] of its name, and
//! the server's reply is what the host's call returned (D § 5). Reads run
//! once more on the next connection if their link drops; writes never
//! (D § 7).

use super::RemoteHost;
use super::conn::{Failed, Retry};
use super::wire;
use fe_host::{
    Conflict, Created, DirEntry, DiskStamp, FsHost, ListReply, ListRequest, Placed, Stat,
    TrashRecord, WriteError, WriteExpect,
};
use fe_remote_proto::{CHUNK, Event, FsData, FsOp, Mux, MuxError, Status, Stream, chan};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

/// Sends `op` on `chan`.
pub(super) fn request<T: serde::Serialize>(mux: &Mux, chan: u16, op: &T) -> Result<Stream, Failed> {
    mux.request(chan, op).map_err(mux_failed)
}

/// A send or a request the mux refused: the link's loss, or a body this
/// side shouldn't have made.
pub(super) fn mux_failed(error: MuxError) -> Failed {
    match error {
        MuxError::LinkLost(_) => Failed::Lost,
        error => Failed::Io(io::Error::other(error)),
    }
}

/// How a reply's `End` ends the call; `Ok` for `Status::Ok`.
pub(super) fn ended(status: Status) -> Result<(), Failed> {
    match status {
        Status::Ok => Ok(()),
        Status::Error(error) => Err(Failed::Io(error.into())),
        // Nothing here cancels a call before its end, but a host that
        // cancelled one gives no outcome.
        Status::Cancelled => Err(Failed::Io(io::Error::new(
            io::ErrorKind::Interrupted,
            "the host cancelled the call",
        ))),
        Status::LinkLost => Err(Failed::Lost),
    }
}

/// Takes the reply's data, each through `take`, until its end.
fn reply(mut stream: Stream, mut take: impl FnMut(FsData) -> io::Result<()>) -> Result<(), Failed> {
    loop {
        match stream.recv() {
            Some(Event::Data(bytes)) => {
                let data = (stream.decode::<FsData>(&bytes)).map_err(wire::invalid)?;
                take(data)?;
            }
            Some(Event::End(status)) => return ended(status),
            // A call's events end with its `End`, the link's loss included.
            None => return Err(Failed::Lost),
        }
    }
}

/// Sends the `bytes` an upload's request announced. A send the mux
/// refuses for anything but the link means the host has ended the reply
/// already: its end says why.
fn upload(stream: &Stream, bytes: &[u8]) -> Result<(), Failed> {
    for chunk in bytes.chunks(CHUNK) {
        match stream.send(&FsData::Bytes(chunk.to_vec())) {
            Ok(()) => {}
            Err(MuxError::LinkLost(_)) => return Err(Failed::Lost),
            Err(_) => break,
        }
    }
    Ok(())
}

fn unexpected(data: &FsData) -> io::Error {
    wire::invalid(format_args!("unexpected {data:?}"))
}

impl RemoteHost {
    /// Runs `op`, taking each body of its reply through `take`, which a
    /// run starts afresh: what a lost run took is dropped.
    fn fs_call<S>(
        &self,
        retry: Retry,
        op: &FsOp,
        upload_bytes: Option<&[u8]>,
        start: impl Fn() -> S,
        take: impl Fn(&mut S, FsData) -> io::Result<()>,
    ) -> io::Result<S> {
        self.conn.run(retry, |mux| {
            let stream = request(mux, chan::FS, op)?;
            if let Some(bytes) = upload_bytes {
                upload(&stream, bytes)?;
            }
            let mut state = start();
            reply(stream, |data| take(&mut state, data))?;
            Ok(state)
        })
    }

    /// Runs `op`, whose reply is one body that `pick` reads.
    fn fs_one<T>(
        &self,
        retry: Retry,
        op: FsOp,
        pick: impl Fn(FsData) -> io::Result<T>,
    ) -> io::Result<T> {
        let got = self.fs_call(
            retry,
            &op,
            None,
            || None,
            |got, data| {
                if got.is_some() {
                    return Err(unexpected(&data));
                }
                *got = Some(pick(data)?);
                Ok(())
            },
        )?;
        got.ok_or_else(|| wire::invalid(format_args!("{op:?} ended with no reply")))
    }

    fn fs_path(&self, retry: Retry, op: FsOp) -> io::Result<PathBuf> {
        self.fs_one(retry, op, |data| match data {
            FsData::Path(path) => Ok(wire::path(&path)),
            data => Err(unexpected(&data)),
        })
    }

    fn fs_placed(&self, op: FsOp) -> io::Result<Placed> {
        self.fs_one(Retry::Never, op, |data| match data {
            FsData::Placed(placed) => Ok(wire::placed(&placed)),
            data => Err(unexpected(&data)),
        })
    }

    fn fs_bool(&self, op: FsOp) -> io::Result<bool> {
        self.fs_one(Retry::Once, op, |data| match data {
            FsData::Bool(found) => Ok(found),
            data => Err(unexpected(&data)),
        })
    }
}

/// A `Read`'s reply as it comes: the file's stamp and length, then its
/// bytes.
#[derive(Default)]
struct File {
    head: Option<(DiskStamp, u64)>,
    bytes: Vec<u8>,
}

impl FsHost for RemoteHost {
    fn read(&self, path: &Path) -> io::Result<(Vec<u8>, DiskStamp)> {
        let op = FsOp::Read {
            path: wire::bytes(path),
        };
        let file = self.fs_call(Retry::Once, &op, None, File::default, |file, data| {
            match (data, &file.head) {
                (FsData::File { stamp, len }, None) => {
                    // At most 4 MiB set aside before the bytes come, so a
                    // length alone can't make this side allocate.
                    let reserve = usize::try_from(len).unwrap_or(0).min(4 << 20);
                    file.bytes.reserve_exact(reserve);
                    file.head = Some((wire::host_stamp(&stamp)?, len));
                }
                (FsData::Bytes(chunk), Some((_, len)))
                    if file.bytes.len() as u64 + chunk.len() as u64 <= *len =>
                {
                    file.bytes.extend_from_slice(&chunk);
                }
                (data, _) => return Err(unexpected(&data)),
            }
            Ok(())
        })?;
        match file.head {
            Some((stamp, len)) if file.bytes.len() as u64 == len => Ok((file.bytes, stamp)),
            _ => Err(wire::invalid("a read ended short of its file")),
        }
    }

    fn write_atomic(
        &self,
        path: &Path,
        bytes: &[u8],
        expect: WriteExpect,
    ) -> Result<DiskStamp, WriteError> {
        let op = FsOp::WriteAtomic {
            path: wire::bytes(path),
            len: bytes.len() as u64,
            expect: wire::expect(expect),
        };
        let written = self.fs_call(
            Retry::Never,
            &op,
            Some(bytes),
            || None,
            |got, data| {
                let outcome = match data {
                    FsData::Written(stamp) if got.is_none() => Ok(wire::host_stamp(&stamp)?),
                    FsData::Conflict { current } if got.is_none() => Err(Conflict {
                        current: current.as_ref().map(wire::host_stamp).transpose()?,
                    }),
                    data => return Err(unexpected(&data)),
                };
                *got = Some(outcome);
                Ok(())
            },
        )?;
        match written {
            Some(Ok(stamp)) => Ok(stamp),
            Some(Err(conflict)) => Err(WriteError::Conflict(conflict)),
            None => Err(wire::invalid("a write ended with no outcome").into()),
        }
    }

    fn stat(&self, path: &Path) -> io::Result<Option<Stat>> {
        let op = FsOp::Stat {
            path: wire::bytes(path),
        };
        self.fs_one(Retry::Once, op, |data| match data {
            FsData::Stat(stat) => stat.as_ref().map(wire::stat).transpose(),
            data => Err(unexpected(&data)),
        })
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        let op = FsOp::ListDir {
            path: wire::bytes(path),
        };
        self.fs_call(
            Retry::Once,
            &op,
            None,
            Vec::new,
            |entries, data| match data {
                FsData::Entries(batch) => {
                    entries.extend(batch.iter().map(wire::dir_entry));
                    Ok(())
                }
                data => Err(unexpected(&data)),
            },
        )
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let op = FsOp::Canonicalize {
            path: wire::bytes(path),
        };
        self.fs_path(Retry::Once, op)
    }

    fn list_tree(&self, request: &ListRequest) -> ListReply {
        // Refused here as the host would, reading nothing.
        if let Err(error) = request.check() {
            return ListReply {
                result: Err(error),
                id: None,
                lost: None,
            };
        }
        let op = FsOp::ListTree(wire::list_tree(request));
        // The entries, then the directory's id; or why it's gone, before
        // the error.
        let mut lost = None;
        let listed = self.conn.run(Retry::Once, |mux| {
            let stream = request_fs(mux, &op)?;
            let mut entries = Vec::new();
            let mut id = None;
            lost = None;
            reply(stream, |data| {
                match data {
                    FsData::Tree(batch) if id.is_none() => {
                        entries.extend(batch.iter().map(wire::entry));
                    }
                    FsData::Listed { id: listed } if id.is_none() => id = Some(listed),
                    FsData::Unlisted { lost: why } => lost = why.map(wire::listing_lost),
                    data => return Err(unexpected(&data)),
                }
                Ok(())
            })?;
            match id {
                Some(id) => Ok((entries, id)),
                None => Err(Failed::Io(wire::invalid("a listing ended with no id"))),
            }
        });
        match listed {
            Ok((entries, id)) => ListReply {
                result: Ok(entries),
                id,
                lost: None,
            },
            Err(error) => ListReply {
                result: Err(error),
                id: None,
                lost,
            },
        }
    }

    fn create_recorded(
        &self,
        root: &Path,
        dir: &Path,
        name: &str,
        contents: &[u8],
        folder: bool,
    ) -> io::Result<Created> {
        let op = FsOp::CreateRecorded {
            root: wire::bytes(root),
            dir: wire::bytes(dir),
            name: name.to_owned(),
            len: contents.len() as u64,
            folder,
        };
        let created = self.fs_call(
            Retry::Never,
            &op,
            Some(contents),
            || None,
            |got, data| match data {
                FsData::Created(created) if got.is_none() => {
                    *got = Some(wire::created(&created));
                    Ok(())
                }
                data => Err(unexpected(&data)),
            },
        )?;
        created.ok_or_else(|| wire::invalid("a create ended with no record"))
    }

    fn rename(&self, root: &Path, from: &Path, new_name: &str) -> io::Result<PathBuf> {
        let op = FsOp::Rename {
            root: wire::bytes(root),
            from: wire::bytes(from),
            new_name: new_name.to_owned(),
        };
        self.fs_path(Retry::Never, op)
    }

    fn duplicate_recorded(&self, root: &Path, path: &Path) -> io::Result<Placed> {
        self.fs_placed(FsOp::DuplicateRecorded {
            root: wire::bytes(root),
            path: wire::bytes(path),
        })
    }

    fn copy_into_recorded(&self, root: &Path, path: &Path, dir: &Path) -> io::Result<Placed> {
        self.fs_placed(FsOp::CopyIntoRecorded {
            root: wire::bytes(root),
            path: wire::bytes(path),
            dir: wire::bytes(dir),
        })
    }

    fn paste_name(
        &self,
        root: &Path,
        path: &Path,
        dir: &Path,
        planned: &[OsString],
    ) -> io::Result<OsString> {
        let op = FsOp::PasteName {
            root: wire::bytes(root),
            path: wire::bytes(path),
            dir: wire::bytes(dir),
            planned: planned.iter().map(|n| n.as_os_str().into()).collect(),
        };
        self.fs_one(Retry::Once, op, |data| match data {
            FsData::Name(name) => Ok(wire::name(&name)),
            data => Err(unexpected(&data)),
        })
    }

    fn move_into(&self, root: &Path, from: &Path, dir: &Path, name: &OsStr) -> io::Result<PathBuf> {
        let op = FsOp::MoveInto {
            root: wire::bytes(root),
            from: wire::bytes(from),
            dir: wire::bytes(dir),
            name: name.into(),
        };
        self.fs_path(Retry::Never, op)
    }

    fn trash_recorded(&self, root: &Path, paths: &[PathBuf]) -> io::Result<Vec<TrashRecord>> {
        let op = FsOp::TrashRecorded {
            root: wire::bytes(root),
            paths: paths.iter().map(|p| wire::bytes(p)).collect(),
        };
        self.fs_call(Retry::Never, &op, None, Vec::new, |records, data| {
            match data {
                FsData::Trashed(batch) => {
                    records.extend(batch.iter().map(wire::host_trash_record));
                }
                data => return Err(unexpected(&data)),
            }
            Ok(())
        })
    }

    /// `false` when the host can't be asked.
    fn restores_from_trash(&self) -> bool {
        self.fs_bool(FsOp::RestoresFromTrash).unwrap_or(false)
    }

    fn restore(&self, root: &Path, trashed: &mut TrashRecord) -> io::Result<PathBuf> {
        let op = FsOp::Restore {
            root: wire::bytes(root),
            trashed: wire::trash_record(trashed),
        };
        // The record as it is now comes first, on failure too.
        let mut record = None;
        let restored = self.conn.run(Retry::Never, |mux| {
            let stream = request_fs(mux, &op)?;
            let mut path = None;
            reply(stream, |data| {
                match data {
                    FsData::Record(now) if record.is_none() => {
                        record = Some(wire::host_trash_record(&now));
                    }
                    FsData::Path(to) if record.is_some() && path.is_none() => {
                        path = Some(wire::path(&to));
                    }
                    data => return Err(unexpected(&data)),
                }
                Ok(())
            })?;
            path.ok_or_else(|| Failed::Io(wire::invalid("a restore ended with no path")))
        });
        if let Some(record) = record {
            *trashed = record;
        }
        restored
    }

    fn in_trash(&self, trashed: &TrashRecord) -> io::Result<bool> {
        self.fs_bool(FsOp::InTrash {
            trashed: wire::trash_record(trashed),
        })
    }

    fn exists(&self, root: &Path, path: &Path) -> io::Result<bool> {
        self.fs_bool(FsOp::Exists {
            root: wire::bytes(root),
            path: wire::bytes(path),
        })
    }

    fn truncate(&self, root: &Path, path: &Path) -> io::Result<()> {
        let op = FsOp::Truncate {
            root: wire::bytes(root),
            path: wire::bytes(path),
        };
        self.fs_call(
            Retry::Never,
            &op,
            None,
            || (),
            |(), data| Err(unexpected(&data)),
        )
    }
}

fn request_fs(mux: &Mux, op: &FsOp) -> Result<Stream, Failed> {
    request(mux, chan::FS, op)
}

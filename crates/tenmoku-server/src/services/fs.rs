//! Channel 1: each [`FsOp`] is the `FsHost` call of the same name, and
//! its reply is what the call returned (D § 5, § 7).
//!
//! Plain reads and writes take any absolute path on the host: the server
//! runs as the user, who may open a file outside the project (D § 9). The
//! tree operations keep fe-project's rules, every path beneath its root.

use super::{absolute, io_failure, pump::mux_failure, send_batched};
use crate::convert;
use fe_host::{FsHost, WriteError};
use fe_remote_proto::{
    CHUNK, Event, FsData, FsOp, Incoming, IoError, IoKind, MuxError, Status, Stream,
};
use std::io;

/// How many bytes of an upload are set aside before they arrive: past
/// this, the buffer grows as they come, so a `len` alone can't make the
/// server allocate.
const UPLOAD_RESERVE: u64 = 16 << 20;

/// Why a request ended early.
enum Failed {
    /// The host's call failed.
    Io(io::Error),
    /// The reply couldn't go on: the client cancelled, or the link went.
    Mux(MuxError),
}

impl From<io::Error> for Failed {
    fn from(e: io::Error) -> Failed {
        Failed::Io(e)
    }
}

impl From<MuxError> for Failed {
    fn from(e: MuxError) -> Failed {
        Failed::Mux(e)
    }
}

/// Whether `incoming` uploads bytes after it: those are read as they
/// come, never queued behind the workers, since bytes waiting unread hold
/// the channel's credit, and a write being served may need that credit
/// for the rest of its own bytes.
pub(super) fn uploads(incoming: &Incoming) -> bool {
    matches!(
        incoming.op::<FsOp>(),
        Ok(FsOp::WriteAtomic { .. } | FsOp::CreateRecorded { .. })
    )
}

pub(super) fn serve(host: &dyn FsHost, incoming: Incoming) {
    let op = match incoming.op::<FsOp>() {
        Ok(op) => op,
        Err(e) => {
            let _ = incoming.stream.end(Status::Error(IoError {
                kind: IoKind::InvalidData,
                message: e.to_string(),
            }));
            return;
        }
    };
    let mut stream = incoming.stream;
    let status = match run(host, op, &mut stream) {
        // A cancel that lands while the host's call runs doesn't undo it,
        // but the request still ends as the client asked.
        Ok(()) if stream.cancelled() => Status::Cancelled,
        Ok(()) => Status::Ok,
        Err(Failed::Io(e)) => io_failure(&e),
        Err(Failed::Mux(MuxError::Cancelled)) => Status::Cancelled,
        // No one is there to answer.
        Err(Failed::Mux(MuxError::LinkLost(_))) => return,
        Err(Failed::Mux(e)) => mux_failure(&e),
    };
    let _ = stream.end(status);
}

/// Refuses to call the host for a request the client has cancelled: one
/// that waited for a worker, or whose upload just ended.
fn go_on(stream: &Stream) -> Result<(), Failed> {
    if stream.cancelled() {
        return Err(Failed::Mux(MuxError::Cancelled));
    }
    Ok(())
}

fn run(host: &dyn FsHost, op: FsOp, stream: &mut Stream) -> Result<(), Failed> {
    go_on(stream)?;
    match op {
        FsOp::Read { path } => {
            let (bytes, stamp) = host.read(&absolute(&path)?)?;
            stream.send(&FsData::File {
                stamp: convert::stamp(&stamp),
                len: bytes.len() as u64,
            })?;
            for chunk in bytes.chunks(CHUNK) {
                stream.send(&FsData::Bytes(chunk.to_vec()))?;
            }
        }
        FsOp::WriteAtomic { path, len, expect } => {
            let path = absolute(&path)?;
            let expect = convert::expect(&expect)?;
            let bytes = upload(stream, len)?;
            go_on(stream)?;
            let reply = match host.write_atomic(&path, &bytes, expect) {
                Ok(stamp) => FsData::Written(convert::stamp(&stamp)),
                Err(WriteError::Conflict(conflict)) => FsData::Conflict {
                    current: conflict.current.as_ref().map(convert::stamp),
                },
                Err(WriteError::Io(e)) => return Err(e.into()),
            };
            stream.send(&reply)?;
        }
        FsOp::Stat { path } => {
            let stat = host.stat(&absolute(&path)?)?;
            stream.send(&FsData::Stat(stat.as_ref().map(convert::stat)))?;
        }
        FsOp::ListDir { path } => {
            let entries = host.list_dir(&absolute(&path)?)?;
            let entries = entries.iter().map(convert::dir_entry);
            send_batched(stream, entries, usize::MAX, FsData::Entries)?;
        }
        FsOp::Canonicalize { path } => {
            let canonical = host.canonicalize(&absolute(&path)?)?;
            stream.send(&FsData::Path(convert::bytes(&canonical)))?;
        }
        FsOp::ListTree(list) => {
            absolute(&list.root)?;
            let reply = host.list_tree(&convert::list_request(&list));
            match reply.result {
                Ok(entries) => {
                    let entries = entries.iter().map(convert::entry);
                    send_batched(stream, entries, usize::MAX, FsData::Tree)?;
                    stream.send(&FsData::Listed { id: reply.id })?;
                }
                Err(e) => {
                    stream.send(&FsData::Unlisted {
                        lost: reply.lost.map(convert::listing_lost),
                    })?;
                    return Err(e.into());
                }
            }
        }
        FsOp::CreateRecorded {
            root,
            dir,
            name,
            len,
            folder,
        } => {
            let root = absolute(&root)?;
            let contents = upload(stream, len)?;
            go_on(stream)?;
            let created =
                host.create_recorded(&root, &convert::path(&dir), &name, &contents, folder)?;
            stream.send(&FsData::Created(convert::created(&created)))?;
        }
        FsOp::Rename {
            root,
            from,
            new_name,
        } => {
            let to = host.rename(&absolute(&root)?, &convert::path(&from), &new_name)?;
            stream.send(&FsData::Path(convert::bytes(&to)))?;
        }
        FsOp::DuplicateRecorded { root, path } => {
            let placed = host.duplicate_recorded(&absolute(&root)?, &convert::path(&path))?;
            stream.send(&FsData::Placed(convert::placed(&placed)))?;
        }
        FsOp::CopyIntoRecorded { root, path, dir } => {
            let placed = host.copy_into_recorded(
                &absolute(&root)?,
                &convert::path(&path),
                &convert::path(&dir),
            )?;
            stream.send(&FsData::Placed(convert::placed(&placed)))?;
        }
        FsOp::PasteName {
            root,
            path,
            dir,
            planned,
        } => {
            let planned: Vec<_> = planned.iter().map(convert::name).collect();
            let name = host.paste_name(
                &absolute(&root)?,
                &convert::path(&path),
                &convert::path(&dir),
                &planned,
            )?;
            stream.send(&FsData::Name(name.as_os_str().into()))?;
        }
        FsOp::MoveInto {
            root,
            from,
            dir,
            name,
        } => {
            let to = host.move_into(
                &absolute(&root)?,
                &convert::path(&from),
                &convert::path(&dir),
                &convert::name(&name),
            )?;
            stream.send(&FsData::Path(convert::bytes(&to)))?;
        }
        FsOp::TrashRecorded { root, paths } => {
            let paths: Vec<_> = paths.iter().map(convert::path).collect();
            let records = host.trash_recorded(&absolute(&root)?, &paths)?;
            let records = records.iter().map(convert::trash_record);
            send_batched(stream, records, usize::MAX, FsData::Trashed)?;
        }
        FsOp::RestoresFromTrash => {
            stream.send(&FsData::Bool(host.restores_from_trash()))?;
        }
        FsOp::Restore { root, trashed } => {
            let root = absolute(&root)?;
            let mut record = convert::host_trash_record(&trashed);
            let restored = host.restore(&root, &mut record);
            // The record follows the entry, on failure too.
            stream.send(&FsData::Record(convert::trash_record(&record)))?;
            stream.send(&FsData::Path(convert::bytes(&restored?)))?;
        }
        FsOp::InTrash { trashed } => {
            let found = host.in_trash(&convert::host_trash_record(&trashed))?;
            stream.send(&FsData::Bool(found))?;
        }
        FsOp::Exists { root, path } => {
            let found = host.exists(&absolute(&root)?, &convert::path(&path))?;
            stream.send(&FsData::Bool(found))?;
        }
        FsOp::Truncate { root, path } => {
            host.truncate(&absolute(&root)?, &convert::path(&path))?;
        }
    }
    Ok(())
}

/// The `len` bytes a `WriteAtomic` or a `CreateRecorded` uploads after it. A cancel, or the
/// link's loss, before the last of them is in writes nothing.
fn upload(stream: &mut Stream, len: u64) -> Result<Vec<u8>, Failed> {
    let reserve = usize::try_from(len.min(UPLOAD_RESERVE)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(reserve);
    while (bytes.len() as u64) < len {
        let data = match stream.recv() {
            Some(Event::Data(data)) => data,
            // A reply's inbox closes on a cancel or a lost link; the peer
            // never ends its own request.
            Some(Event::End(_)) | None => return Err(Failed::Mux(MuxError::Cancelled)),
        };
        let chunk = match stream.decode::<FsData>(&data) {
            Ok(FsData::Bytes(chunk)) => chunk,
            Ok(_) => return Err(invalid("an upload sent something other than bytes".into()).into()),
            Err(e) => return Err(invalid(e.to_string()).into()),
        };
        if bytes.len() as u64 + chunk.len() as u64 > len {
            return Err(invalid(format!("an upload sent more than its {len} bytes")).into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

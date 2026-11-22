//! Channel 3: a `Search` or a `Files` is the `SearchHost` call of that
//! name, its stream's batches sent as they come (D § 2.2, § 5). A cancel
//! sets the host stream's cancel flag at once, so the host's search stops
//! between files rather than at its next batch.

use super::pump::{Mailbox, Pumped, pump};
use super::{absolute, io_failure, send_batched};
use crate::convert;
use fe_host::{SearchFailed, SearchHost};
use fe_remote_proto::{
    CHUNK, FILES_BATCH, FileMatches, Incoming, IoError, IoKind, LineMatch, MuxError, PathBytes,
    SearchData, SearchFailure, SearchOp, Status, Stream, encoded_len,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

pub(super) fn serve(host: &dyn SearchHost, incoming: Incoming) {
    let op = match incoming.op::<SearchOp>() {
        Ok(op) => op,
        Err(e) => {
            let _ = incoming.stream.end(Status::Error(IoError {
                kind: IoKind::InvalidData,
                message: e.to_string(),
            }));
            return;
        }
    };
    let reply = incoming.stream;
    match op {
        SearchOp::Search {
            root,
            query,
            options,
            filter,
            excluded,
        } => {
            let root = match absolute(&root) {
                Ok(root) => root,
                Err(e) => return drop(reply.end(io_failure(&e))),
            };
            let found = host.search(
                &root,
                &convert::query(&query),
                &convert::options(&options),
                &convert::filter(&filter, excluded.as_ref()),
            );
            stream(reply, found, send_matches, |reply, end| match end {
                Some(Ok(summary)) => match send_summary(reply, &summary) {
                    Ok(()) => Status::Ok,
                    Err(e) => sent_status(&e),
                },
                Some(Err(failed)) => failure(reply, failed),
                None => stopped_short(),
            });
        }
        SearchOp::Files {
            root,
            filter,
            excluded,
        } => {
            let root = match absolute(&root) {
                Ok(root) => root,
                Err(e) => return drop(reply.end(io_failure(&e))),
            };
            let listed = host.files(&root, &convert::filter(&filter, excluded.as_ref()));
            // The host's batches are as its walk threads fill them: what
            // doesn't make up a batch of its own waits for the next.
            let held = RefCell::new(Vec::new());
            stream(
                reply,
                listed,
                |reply, batch| send_files(reply, &mut held.borrow_mut(), batch),
                |reply, end| match end {
                    Some(Ok(_)) => match flush_files(reply, &mut held.borrow_mut()) {
                        Ok(()) => Status::Ok,
                        Err(e) => sent_status(&e),
                    },
                    Some(Err(failed)) => failure(reply, failed),
                    None => stopped_short(),
                },
            );
        }
    }
}

/// Serves `source`, a host's stream, on `reply` until it ends or the
/// client cancels, when the host's work is cancelled too.
fn stream<X: Send + 'static, E: Send + 'static>(
    mut reply: Stream,
    source: fe_host::Stream<Vec<X>, E>,
    deliver: impl FnMut(&Stream, Vec<X>) -> Result<(), MuxError>,
    finish: impl FnOnce(&Stream, Option<E>) -> Status,
) {
    let cancel = source.cancel_flag();
    let mailbox = Mailbox::<X, E>::new();
    let forwarding = mailbox.clone();
    let started = std::thread::Builder::new()
        .name("search-forward".into())
        .spawn(move || {
            while let Some(batch) = source.recv() {
                if !forwarding.put(batch) {
                    // Dropping the source cancels it.
                    return;
                }
            }
            forwarding.end(source.finish());
        });
    if let Err(e) = started {
        let _ = reply.end(io_failure(&e));
        return;
    }
    let pumped = pump(&mut reply, &mailbox, deliver, finish);
    // However the reply ended, the host has nothing more to do: a search
    // that ran to its end has stopped already.
    cancel.store(true, Ordering::Relaxed);
    if pumped == Pumped::Stopped {
        let _ = reply.end(Status::Cancelled);
    }
}

fn send_matches(reply: &Stream, batch: Vec<fe_host::FileMatches>) -> Result<(), MuxError> {
    let files = batch
        .iter()
        .map(convert::file_matches)
        .flat_map(split_to_fit);
    send_batched(reply, files, usize::MAX, SearchData::Matches)
}

/// Sends the whole batches of [`FILES_BATCH`] paths `held` and `batch`
/// make, keeping the rest in `held`.
fn send_files(
    reply: &Stream,
    held: &mut Vec<PathBytes>,
    batch: Vec<PathBuf>,
) -> Result<(), MuxError> {
    held.extend(batch.iter().map(|p| convert::bytes(p)));
    let whole = held.len() - held.len() % FILES_BATCH;
    send_batched(reply, held.drain(..whole), FILES_BATCH, SearchData::Files)
}

fn flush_files(reply: &Stream, held: &mut Vec<PathBytes>) -> Result<(), MuxError> {
    send_batched(reply, held.drain(..), FILES_BATCH, SearchData::Files)
}

/// Room in a frame's body for what isn't a file's matches: the body's
/// tag and the lengths of its lists.
const FRAMING: usize = 16;

/// `file` as consecutive entries of the same path that each fit in a
/// frame with its path and rel: its lines in order, and a line with too
/// many ranges for one as consecutive entries of that line.
fn split_to_fit(file: FileMatches) -> Vec<FileMatches> {
    if size(&file) + FRAMING <= CHUNK {
        return vec![file];
    }
    let FileMatches { path, rel, lines } = file;
    let envelope = size(&FileMatches {
        path: path.clone(),
        rel: rel.clone(),
        lines: Vec::new(),
    });
    let room = CHUNK.saturating_sub(envelope + FRAMING).max(1);
    let mut pieces = Vec::new();
    let mut lines_in = Vec::new();
    let mut taken = 0;
    for line in lines.into_iter().flat_map(|line| split_line(line, room)) {
        let len = size(&line);
        if !lines_in.is_empty() && taken + len > room {
            pieces.push(std::mem::take(&mut lines_in));
            taken = 0;
        }
        taken += len;
        lines_in.push(line);
    }
    pieces.push(lines_in);
    (pieces.into_iter())
        .map(|lines| FileMatches {
            path: path.clone(),
            rel: rel.clone(),
            lines,
        })
        .collect()
}

/// `line` as consecutive entries of that line that each take at most
/// `room`.
fn split_line(line: LineMatch, room: usize) -> Vec<LineMatch> {
    if size(&line) <= room {
        return vec![line];
    }
    let bare = LineMatch {
        ranges: Vec::new(),
        ..line.clone()
    };
    let room = room.saturating_sub(size(&bare) + FRAMING).max(1);
    let mut pieces = Vec::new();
    let mut ranges = Vec::new();
    let mut taken = 0;
    for range in line.ranges {
        let len = size(&range);
        if !ranges.is_empty() && taken + len > room {
            pieces.push(std::mem::take(&mut ranges));
            taken = 0;
        }
        taken += len;
        ranges.push(range);
    }
    pieces.push(ranges);
    (pieces.into_iter())
        .map(|ranges| LineMatch {
            ranges,
            ..bare.clone()
        })
        .collect()
}

/// The longest message a failure carries, in bytes: a pattern or a glob
/// may be as long as a request, and its message quotes it.
const MESSAGE_MOST: usize = 4 * 1024;

/// `text`, cut to at most [`MESSAGE_MOST`] bytes on a char boundary.
fn clip(text: &str) -> String {
    if text.len() <= MESSAGE_MOST {
        return text.to_owned();
    }
    let mut end = MESSAGE_MOST - '…'.len_utf8();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// A search's summary: the files it couldn't search in `Errors` batches,
/// then the rest, whose `errors` are those.
fn send_summary(reply: &Stream, summary: &fe_host::SearchSummary) -> Result<(), MuxError> {
    let mut summary = convert::summary(summary);
    let errors = std::mem::take(&mut summary.errors);
    let errors = (errors.into_iter()).map(|(path, message)| (path, clip(&message)));
    send_batched(reply, errors, usize::MAX, SearchData::Errors)?;
    reply.send(&SearchData::Summary(summary))
}

fn size<T: serde::Serialize>(value: &T) -> usize {
    // These types always encode.
    encoded_len(value).unwrap_or(usize::MAX)
}

/// How a search that couldn't run ends: what failed, then the error.
fn failure(reply: &Stream, failed: SearchFailed) -> Status {
    let failure = match &failed {
        SearchFailed::InvalidPattern(message) => SearchFailure::InvalidPattern(clip(message)),
        SearchFailed::InvalidGlob { glob, message } => SearchFailure::InvalidGlob {
            glob: clip(glob),
            message: clip(message),
        },
        SearchFailed::Io(e) => return io_failure(e),
    };
    if let Err(e) = reply.send(&SearchData::Failed(failure)) {
        return sent_status(&e);
    }
    Status::Error(IoError {
        kind: IoKind::InvalidInput,
        message: clip(&failed.to_string()),
    })
}

/// The status a reply ends with when its last data couldn't go.
fn sent_status(e: &MuxError) -> Status {
    match e {
        MuxError::Cancelled | MuxError::LinkLost(_) => Status::Cancelled,
        e => super::pump::mux_failure(e),
    }
}

/// The host's stream ended with no value: its work stopped short (it
/// panicked, say).
fn stopped_short() -> Status {
    Status::Error(IoError {
        kind: IoKind::Other,
        message: "the host's search stopped without a result".into(),
    })
}

//! Channel 2: a `Watch` is a `WatchHost::watch` subscription, streamed to
//! the client batch by batch until it cancels (D § 2.2, § 5). The batches
//! are the host's `FsChange`s, the wire type unchanged.
//!
//! The host's batches are never held up (as `WatchHost` has them), so
//! while the client's credit is spent they merge into one waiting batch.
//! Past [`PENDING_MOST`] changes that batch becomes a single `Rescan`,
//! which covers whatever else changes until it is sent.

use super::pump::{Mailbox, Pumped, pump};
use super::{absolute, io_failure, send_batched};
use crate::convert;
use fe_host::{FsChange, WatchHost};
use fe_remote_proto::{Incoming, IoError, IoKind, Status, WatchOp};

/// The changes that may wait for the client before they become a `Rescan`.
const PENDING_MOST: usize = 10_000;

pub(super) fn serve(host: &dyn WatchHost, incoming: Incoming) {
    let root = match incoming.op::<WatchOp>() {
        Ok(WatchOp::Watch { root }) => absolute(&root),
        Err(e) => {
            let _ = incoming.stream.end(Status::Error(IoError {
                kind: IoKind::InvalidData,
                message: e.to_string(),
            }));
            return;
        }
    };
    let mut reply = incoming.stream;
    let subscription = match root.and_then(|root| host.watch(&root)) {
        Ok(subscription) => subscription,
        Err(e) => {
            let _ = reply.end(io_failure(&e));
            return;
        }
    };
    let (changes, source) = subscription.into_parts();
    let mailbox = Mailbox::<FsChange, ()>::new();
    let forwarding = mailbox.clone();
    let started = std::thread::Builder::new()
        .name("watch-forward".into())
        .spawn(move || {
            while let Ok(batch) = changes.recv() {
                if !forwarding.merge(batch, coalesce) {
                    return;
                }
            }
            forwarding.end(None);
        });
    if let Err(e) = started {
        let _ = reply.end(io_failure(&e));
        return;
    }
    // The watch is in place: a change from now on is reported.
    let _ = reply.send(&Vec::<fe_remote_proto::FsChange>::new());
    let pumped = pump(
        &mut reply,
        &mailbox,
        |reply, batch| {
            let batch = batch.iter().map(convert::fs_change);
            send_batched(reply, batch, usize::MAX, |batch| batch)
        },
        // The host stopped watching of its own accord.
        |_, _| Status::Ok,
    );
    // Stops the watch, which ends the forwarder's wait.
    drop(source);
    if pumped == Pumped::Stopped {
        let _ = reply.end(Status::Cancelled);
    }
}

/// Merges the host's `batch` into the changes waiting.
fn coalesce(pending: &mut Vec<FsChange>, batch: Vec<FsChange>) {
    if pending.is_empty() {
        *pending = batch;
    } else if pending[..] != [FsChange::Rescan] {
        pending.extend(batch);
    }
    if pending.len() > PENDING_MOST {
        *pending = vec![FsChange::Rescan];
    }
}

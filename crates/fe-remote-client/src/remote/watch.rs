//! [`WatchHost`] over channel 2: a `Watch` request that stays open, each
//! of its `Data` frames a batch of changes (D § 5).
//!
//! The server's first batch is empty and says the watch is in place, so
//! `watch` returns once it has come, as the local impl returns once its
//! watcher is: a change made after that is reported. A thread then feeds
//! the subscription until it is dropped, which cancels the request. When
//! the link drops, the thread watches again on the next connection and
//! delivers a [`FsChange::Rescan`], since what changed meanwhile isn't
//! known (phase 9's resume replays the session's dirty set instead).

use super::conn::{Attempt, Conn, Failed, Retry};
use super::fs::{ended, request};
use super::{RemoteHost, Wakeup, wire};
use fe_host::{FsChange, Subscription, WatchHost};
use fe_remote_proto::{Event, Status, Stream, WatchOp, chan};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::task::{Context, Poll, Waker};

impl WatchHost for RemoteHost {
    fn watch(&self, root: &Path) -> io::Result<Subscription<Vec<FsChange>>> {
        let op = WatchOp::Watch {
            root: wire::bytes(root),
        };
        let (stream, attempt) = self
            .conn
            .run_on(Retry::Once, |attempt| start(attempt, &op))?;
        let (tx, rx) = mpsc::channel();
        let wake = Arc::new(Wakeup::default());
        let watching = Watching {
            conn: self.conn.clone(),
            op,
            attempt,
            stream,
            tx,
            wake: wake.clone(),
        };
        std::thread::Builder::new()
            .name("remote-watch".into())
            .spawn(move || watching.forward())?;
        Ok(Subscription::new(rx, Unwatch(wake)))
    }
}

/// Sends the watch and waits for the server's first, empty batch.
fn start(attempt: &Attempt, op: &WatchOp) -> Result<Stream, Failed> {
    let mut stream = request(attempt.mux(), chan::WATCH, op)?;
    match stream.recv() {
        Some(Event::Data(_)) => Ok(stream),
        Some(Event::End(status)) => {
            ended(status)?;
            Err(Failed::Io(wire::invalid("a watch ended as it started")))
        }
        None => Err(Failed::Lost),
    }
}

/// A subscription's source: dropping it stops the forwarding thread,
/// which drops the request, cancelling it.
struct Unwatch(Arc<Wakeup>);

impl Drop for Unwatch {
    fn drop(&mut self) {
        self.0.stop();
    }
}

/// One watch's forwarding thread.
struct Watching {
    conn: Conn,
    op: WatchOp,
    attempt: Attempt,
    stream: Stream,
    tx: Sender<Vec<FsChange>>,
    wake: Arc<Wakeup>,
}

impl Watching {
    fn forward(mut self) {
        let waker = Waker::from(self.wake.clone());
        let mut cx = Context::from_waker(&waker);
        while !self.wake.stopped() {
            match self.stream.poll_recv(&mut cx) {
                Poll::Pending => self.wake.wait(None),
                Poll::Ready(Some(Event::Data(bytes))) => {
                    let Ok(batch) = self.stream.decode::<Vec<fe_remote_proto::FsChange>>(&bytes)
                    else {
                        return;
                    };
                    if batch.is_empty() {
                        continue;
                    }
                    let batch = batch.iter().map(wire::fs_change).collect();
                    if self.tx.send(batch).is_err() {
                        return;
                    }
                }
                Poll::Ready(Some(Event::End(Status::LinkLost)) | None) => {
                    if !self.rewatch() {
                        return;
                    }
                }
                // The host stopped watching, or failed to: nothing more
                // comes.
                Poll::Ready(Some(Event::End(_))) => return,
            }
        }
    }

    /// Watches again on the next connection, which a change during the
    /// gap may have missed: false if there's none, or the host refuses.
    fn rewatch(&mut self) -> bool {
        loop {
            let wake = self.wake.clone();
            let next = self.conn.next_unless(&self.attempt, || wake.stopped());
            let Ok(Some(next)) = next else {
                return false;
            };
            self.attempt = next;
            match start(&self.attempt, &self.op) {
                Ok(stream) => {
                    self.stream = stream;
                    return self.tx.send(vec![FsChange::Rescan]).is_ok();
                }
                Err(Failed::Lost) => {}
                Err(Failed::Io(_)) => return false,
            }
        }
    }
}

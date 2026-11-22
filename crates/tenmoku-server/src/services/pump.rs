//! A subscription or a stream served from a host's results: what the
//! watch and search services share.
//!
//! The host's results arrive on a thread of their own (the forwarder),
//! which puts them in a [`Mailbox`]. The request's thread takes them out
//! and sends them. A `Cancel`, or the link's loss, wakes that thread
//! through the reply's waker, wherever it waits, so a watch on a quiet tree
//! or a search between matches stops at once rather than at its next
//! result.

use fe_remote_proto::{IoError, IoKind, MuxError, Status, Stream};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll, Wake, Waker};

/// Results on their way from a host's forwarder to the request's thread:
/// `X`s, then the value the host's work ended with.
pub(crate) struct Mailbox<X, E> {
    state: Mutex<Inbox<X, E>>,
    changed: Condvar,
}

struct Inbox<X, E> {
    items: Vec<X>,
    /// The host's work is done, with this value if it gave one.
    end: Option<Option<E>>,
    /// The reply's waker went off: look at the reply.
    woken: bool,
    /// The request's thread has gone: nothing more is taken.
    closed: bool,
}

/// What [`Mailbox::take`] found.
pub(crate) enum Took<X, E> {
    Items(Vec<X>),
    End(Option<E>),
    Woken,
}

impl<X, E> Mailbox<X, E> {
    pub(crate) fn new() -> Arc<Mailbox<X, E>> {
        Arc::new(Mailbox {
            state: Mutex::new(Inbox {
                items: Vec::new(),
                end: None,
                woken: false,
                closed: false,
            }),
            changed: Condvar::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inbox<X, E>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Adds `batch` once the last has been taken, so a slow client holds
    /// the host up. False once the request's thread has gone.
    pub(crate) fn put(&self, batch: Vec<X>) -> bool {
        let mut st = self.lock();
        while !st.items.is_empty() && !st.closed {
            st = self.changed.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        if st.closed {
            return false;
        }
        st.items = batch;
        self.changed.notify_all();
        true
    }

    /// Merges `batch` into what waits, never waiting: a host whose results
    /// mustn't be held up. False once the request's thread has gone.
    pub(crate) fn merge(&self, batch: Vec<X>, merge: impl FnOnce(&mut Vec<X>, Vec<X>)) -> bool {
        let mut st = self.lock();
        if st.closed {
            return false;
        }
        merge(&mut st.items, batch);
        self.changed.notify_all();
        true
    }

    /// The host's work is done, ending with `value`.
    pub(crate) fn end(&self, value: Option<E>) {
        self.lock().end = Some(value);
        self.changed.notify_all();
    }

    fn close(&self) {
        let mut st = self.lock();
        st.closed = true;
        st.items.clear();
        self.changed.notify_all();
    }

    fn wake(&self) {
        self.lock().woken = true;
        self.changed.notify_all();
    }

    /// The next thing to act on, waiting for one: items before the end.
    fn take(&self) -> Took<X, E> {
        let mut st = self.lock();
        loop {
            if std::mem::take(&mut st.woken) {
                return Took::Woken;
            }
            if !st.items.is_empty() {
                let items = std::mem::take(&mut st.items);
                self.changed.notify_all();
                return Took::Items(items);
            }
            if let Some(end) = st.end.take() {
                return Took::End(end);
            }
            st = self.changed.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }
}

struct Wakeup<X, E>(Arc<Mailbox<X, E>>);

impl<X: Send, E: Send> Wake for Wakeup<X, E> {
    fn wake(self: Arc<Self>) {
        self.0.wake();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.wake();
    }
}

/// How a pump ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pumped {
    /// The reply has ended.
    Ended,
    /// The client cancelled, or the link was lost: stop the host's work,
    /// and end the reply with `Cancelled`.
    Stopped,
}

/// Sends what `mailbox` gets, through `deliver`, until the host's work is
/// done, when `finish` sends what it ended with and says how the reply
/// ends; or until the client cancels. Either way the mailbox is closed
/// once this returns, so the forwarder's next put fails.
pub(crate) fn pump<X: Send + 'static, E: Send + 'static>(
    reply: &mut Stream,
    mailbox: &Arc<Mailbox<X, E>>,
    mut deliver: impl FnMut(&Stream, Vec<X>) -> Result<(), MuxError>,
    finish: impl FnOnce(&Stream, Option<E>) -> Status,
) -> Pumped {
    let waker = Waker::from(Arc::new(Wakeup(mailbox.clone())));
    let mut cx = Context::from_waker(&waker);
    let pumped = loop {
        match reply.poll_recv(&mut cx) {
            // A reply's inbox closes when the client cancels or the link
            // goes.
            Poll::Ready(None) => break Pumped::Stopped,
            // Nothing is uploaded to these requests: what came is
            // dropped, and its credit returned.
            Poll::Ready(Some(_)) => continue,
            Poll::Pending => {}
        }
        match mailbox.take() {
            Took::Woken => {}
            Took::Items(items) => match deliver(reply, items) {
                // The next look at the reply finds why.
                Ok(()) | Err(MuxError::Cancelled | MuxError::LinkLost(_)) => {}
                Err(e) => {
                    let _ = reply.end(mux_failure(&e));
                    break Pumped::Ended;
                }
            },
            Took::End(value) => {
                let status = finish(reply, value);
                let _ = reply.end(status);
                break Pumped::Ended;
            }
        }
    };
    mailbox.close();
    pumped
}

/// The status for a reply that couldn't be sent for a reason of this
/// side's own.
pub(crate) fn mux_failure(e: &MuxError) -> Status {
    Status::Error(IoError {
        kind: IoKind::Other,
        message: format!("the reply couldn't be sent: {e}"),
    })
}

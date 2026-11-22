//! [`RemoteHost`]: fe-host's `FsHost`, `WatchHost` and `SearchHost` over
//! a connection to the host's tenmoku-server (D § 5: a remote window's
//! `HostSet` is `RemoteHost(conn)`), on channels 1, 2 and 3.
//!
//! Each call blocks, as the traits' do, so callers run it off the UI
//! thread (D § 11: nothing on it awaits the network). A call is one
//! request. When its link drops, a read waits for the next connection
//! ([`Conn::reconnected`]) and runs once more on it; a write fails with
//! [`LinkError::Lost`], as its outcome isn't known (D § 7). A watch
//! watches again on each next connection, and a search or file list that
//! has delivered nothing yet runs again once. Dropping a watch's
//! subscription or a search's stream cancels its request.

mod conn;
mod fs;
mod search;
mod watch;
mod wire;

pub use conn::{Conn, LinkError};

use fe_host::HostSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Wake;
use std::time::Duration;

/// A host reached over a connection to its tenmoku-server. Cheap to
/// clone: every clone calls over the same [`Conn`].
#[derive(Clone, Debug)]
pub struct RemoteHost {
    conn: Conn,
}

impl RemoteHost {
    pub fn new(conn: Conn) -> RemoteHost {
        RemoteHost { conn }
    }

    pub fn conn(&self) -> &Conn {
        &self.conn
    }

    /// A remote window's hosts: this one for each trait.
    pub fn host_set(&self) -> HostSet {
        HostSet {
            fs: Arc::new(self.clone()),
        }
    }
}

/// What a thread polling a reply waits on: the reply's waker, and a stop
/// from whoever dropped what it feeds.
#[derive(Default)]
struct Wakeup {
    woken: Mutex<bool>,
    changed: Condvar,
    stopped: AtomicBool,
}

impl Wakeup {
    /// Waits for a wake since the last, at most `timeout` if given.
    fn wait(&self, timeout: Option<Duration>) {
        let mut woken = self.woken.lock().unwrap_or_else(|e| e.into_inner());
        if !*woken {
            woken = match timeout {
                Some(timeout) => {
                    (self.changed.wait_timeout(woken, timeout))
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                }
                None => (self.changed.wait(woken)).unwrap_or_else(|e| e.into_inner()),
            };
        }
        *woken = false;
    }

    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.wake_up();
    }

    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn wake_up(&self) {
        *self.woken.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.changed.notify_all();
    }
}

impl Wake for Wakeup {
    fn wake(self: Arc<Self>) {
        self.wake_up();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_up();
    }
}

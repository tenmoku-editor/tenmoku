//! [`Conn`]: the connection a window's [`super::RemoteHost`] works over
//! (D § 5's `RemoteHost(conn)`), and the next one once the link drops
//! (D § 7, "On resume").

use crate::Connection;
use fe_remote_proto::{Lost, Mux};
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// How often work that waits on the link looks whether it is still
/// wanted, where nothing tells it: a search whose stream was dropped.
pub(crate) const STOP_POLL: Duration = Duration::from_millis(20);

/// The connection a window's remote hosts call over: the one connected
/// now, replaced by whoever reconnects the window when its link drops. A
/// read that lost its link waits for the next connection and runs once
/// more on it; a write never does (D § 7). Cheap to clone: every clone is
/// the same connection.
#[derive(Clone)]
pub struct Conn(Arc<Slot>);

struct Slot {
    state: Mutex<Current>,
    changed: Condvar,
}

struct Current {
    connection: Option<Arc<Connection>>,
    /// Counts the connections put in, so a call knows whether the one it
    /// lost has been replaced.
    generation: u64,
    /// No next connection is coming.
    disconnected: bool,
}

/// Why a remote call failed for the link rather than for the host: the
/// inner error of the [`io::Error`] it fails with, for the caller to word
/// ([`LinkError::of`]), such as a tab's "Not loaded — reconnecting" or a
/// save's *save pending* (D § 11).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    /// The link dropped while the call was in flight, or before it went:
    /// a write, whose outcome isn't known (it may have been committed), or
    /// a read whose one retry lost its link too. `ConnectionAborted`.
    #[error("the connection to the host was lost: {0}")]
    Lost(Lost),
    /// No next connection is coming: the window is disconnected.
    /// `NotConnected`.
    #[error("not connected to the host")]
    Disconnected,
}

impl LinkError {
    /// The [`LinkError`] `error` carries, if it does.
    pub fn of(error: &io::Error) -> Option<&LinkError> {
        error.get_ref()?.downcast_ref()
    }
}

impl From<LinkError> for io::Error {
    fn from(error: LinkError) -> io::Error {
        let kind = match error {
            LinkError::Lost(_) => io::ErrorKind::ConnectionAborted,
            LinkError::Disconnected => io::ErrorKind::NotConnected,
        };
        io::Error::new(kind, error)
    }
}

/// Whether a call may run again on the next connection once its link is
/// lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retry {
    /// A read: running it twice changes nothing on the host.
    Once,
    /// A write, or anything else that changes the host: its first run may
    /// have done it.
    Never,
}

/// How one run of a call went.
pub(crate) enum Failed {
    /// The host answered with an error, or the reply made no sense.
    Io(io::Error),
    /// The link went before the reply's end.
    Lost,
}

impl From<io::Error> for Failed {
    fn from(error: io::Error) -> Failed {
        Failed::Io(error)
    }
}

/// One connection a call runs on: which it is, and the connection.
pub(crate) struct Attempt {
    pub(crate) generation: u64,
    pub(crate) connection: Arc<Connection>,
}

impl Attempt {
    pub(crate) fn mux(&self) -> &Mux {
        self.connection.mux()
    }

    /// The error for a call that lost this attempt's link.
    pub(crate) fn lost(&self) -> io::Error {
        LinkError::Lost(self.mux().lost().unwrap_or(Lost::Eof)).into()
    }
}

impl Conn {
    pub fn new(connection: Connection) -> Conn {
        Conn(Arc::new(Slot {
            state: Mutex::new(Current {
                connection: Some(Arc::new(connection)),
                generation: 0,
                disconnected: false,
            }),
            changed: Condvar::new(),
        }))
    }

    fn lock(&self) -> MutexGuard<'_, Current> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The connection calls go over now; `None` once disconnected.
    pub fn current(&self) -> Option<Arc<Connection>> {
        self.lock().connection.clone()
    }

    /// The link is back: `connection` replaces the one that was lost, and
    /// the reads waiting for it run again on it.
    pub fn reconnected(&self, connection: Connection) {
        let mut st = self.lock();
        st.connection = Some(Arc::new(connection));
        st.generation += 1;
        st.disconnected = false;
        self.0.changed.notify_all();
    }

    /// No next connection is coming (the grace ended, say, or the window
    /// closed): the reads waiting for one fail with
    /// [`LinkError::Disconnected`], as does every call until
    /// [`Conn::reconnected`].
    pub fn disconnected(&self) {
        let mut st = self.lock();
        st.connection = None;
        st.disconnected = true;
        self.0.changed.notify_all();
    }

    /// The connection now.
    pub(crate) fn live(&self) -> io::Result<Attempt> {
        let st = self.lock();
        match &st.connection {
            Some(connection) => Ok(Attempt {
                generation: st.generation,
                connection: connection.clone(),
            }),
            None => Err(LinkError::Disconnected.into()),
        }
    }

    /// The connection after `attempt`'s, waiting for one if need be.
    pub(crate) fn next(&self, attempt: &Attempt) -> io::Result<Attempt> {
        let mut st = self.lock();
        loop {
            if let Some(next) = Self::after(&st, attempt)? {
                return Ok(next);
            }
            st = (self.0.changed.wait(st)).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// [`Conn::next`], or `None` once `stopped` says so: it's asked every
    /// [`STOP_POLL`] while this waits.
    pub(crate) fn next_unless(
        &self,
        attempt: &Attempt,
        stopped: impl Fn() -> bool,
    ) -> io::Result<Option<Attempt>> {
        let mut st = self.lock();
        loop {
            if let Some(next) = Self::after(&st, attempt)? {
                return Ok(Some(next));
            }
            if stopped() {
                return Ok(None);
            }
            st = (self.0.changed.wait_timeout(st, STOP_POLL))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    fn after(st: &Current, attempt: &Attempt) -> io::Result<Option<Attempt>> {
        if st.disconnected {
            return Err(LinkError::Disconnected.into());
        }
        Ok(match &st.connection {
            Some(connection) if st.generation != attempt.generation => Some(Attempt {
                generation: st.generation,
                connection: connection.clone(),
            }),
            _ => None,
        })
    }

    /// Runs `call` on the connection, and, for [`Retry::Once`], once more
    /// on the next connection should the first lose its link. A call that
    /// loses its link and isn't run again fails with [`LinkError::Lost`].
    pub(crate) fn run<T>(
        &self,
        retry: Retry,
        mut call: impl FnMut(&Mux) -> Result<T, Failed>,
    ) -> io::Result<T> {
        self.run_on(retry, |attempt| call(attempt.mux()))
            .map(|(value, _)| value)
    }

    /// [`Conn::run`], saying which connection the call ended on.
    pub(crate) fn run_on<T>(
        &self,
        retry: Retry,
        mut call: impl FnMut(&Attempt) -> Result<T, Failed>,
    ) -> io::Result<(T, Attempt)> {
        let mut attempt = self.live()?;
        let mut retried = false;
        loop {
            match call(&attempt) {
                Ok(value) => return Ok((value, attempt)),
                Err(Failed::Io(error)) => return Err(error),
                Err(Failed::Lost) if retry == Retry::Never || retried => {
                    return Err(attempt.lost());
                }
                Err(Failed::Lost) => {
                    attempt = self.next(&attempt)?;
                    retried = true;
                }
            }
        }
    }
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn").finish_non_exhaustive()
    }
}

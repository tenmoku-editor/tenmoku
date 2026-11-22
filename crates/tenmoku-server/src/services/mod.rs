//! The services on the fixed channels (D § 2.2): 1 fs, 2 watch and 3
//! search. The server is the host side of fe-host's `FsHost`, `WatchHost`
//! and `SearchHost` (D § 5): each request is one call of the session's
//! [`Hosts`], fe-project's local impls unless a test gives its own, and
//! its reply is what that call returned.
//!
//! An fs request runs on one of the session's few fs workers, but for one
//! that uploads its bytes, which takes them on a thread of its own. A watch or a
//! search runs on a thread of its own for as long as it streams, and stops
//! when the client cancels it or the link goes.

mod fs;
mod pool;
mod pump;
mod search;
mod watch;

use fe_host::{FsHost, SearchHost, WatchHost};
use fe_remote_proto::{CHUNK, Incoming, IoError, IoKind, MuxError, Status, Stream, encoded_len};
use pool::Pool;
use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The fs workers a session may run at once. A write or a create, which
/// uploads, runs on a thread of its own instead.
pub const FS_WORKERS: usize = 8;

/// The host impls a session's requests call.
#[derive(Clone)]
pub struct Hosts {
    pub fs: Arc<dyn FsHost>,
    pub watch: Arc<dyn WatchHost>,
    pub search: Arc<dyn SearchHost>,
}

impl Hosts {
    /// This machine's: fe-project's `LocalTreeFs`, `LocalWatch` and
    /// `LocalSearch`, as a local window has them.
    pub fn local() -> Hosts {
        Hosts {
            fs: Arc::new(fe_project::LocalTreeFs::new()),
            watch: Arc::new(fe_project::LocalWatch),
            search: Arc::new(fe_project::LocalSearch),
        }
    }
}

/// One session's services, for as long as its link serves it.
pub(crate) struct Services {
    hosts: Hosts,
    fs_workers: Pool,
}

impl Services {
    pub(crate) fn new(hosts: Hosts) -> Services {
        Services {
            hosts,
            fs_workers: Pool::new("fs-service", FS_WORKERS),
        }
    }

    /// Starts serving a request on a fixed service channel.
    pub(crate) fn serve(&self, incoming: Incoming) {
        use fe_remote_proto::chan;
        match incoming.header.chan {
            chan::FS if fs::uploads(&incoming) => {
                let host = self.hosts.fs.clone();
                on_a_thread("fs-upload", incoming, move |incoming| {
                    fs::serve(&*host, incoming);
                });
            }
            chan::FS => {
                let host = self.hosts.fs.clone();
                let job = Box::new(move || fs::serve(&*host, incoming));
                if let Err(job) = self.fs_workers.run(job) {
                    // No worker could start: answer here rather than not at
                    // all, holding up the requests behind it.
                    job();
                }
            }
            chan::WATCH => {
                let host = self.hosts.watch.clone();
                on_a_thread("watch-service", incoming, move |incoming| {
                    watch::serve(&*host, incoming);
                });
            }
            chan::SEARCH => {
                let host = self.hosts.search.clone();
                on_a_thread("search-service", incoming, move |incoming| {
                    search::serve(&*host, incoming);
                });
            }
            c => {
                let _ = incoming.stream.end(Status::Error(IoError {
                    kind: IoKind::Unsupported,
                    message: format!("this tenmoku-server has no service on channel {c}"),
                }));
            }
        }
    }
}

/// Runs `serve` on a thread named `name`, or ends the request with why
/// there is none.
fn on_a_thread(name: &str, incoming: Incoming, serve: impl FnOnce(Incoming) + Send + 'static) {
    // The request comes back if the thread doesn't start.
    let slot = Arc::new(Mutex::new(Some(incoming)));
    let taken = slot.clone();
    let started = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let incoming = taken.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(incoming) = incoming {
                serve(incoming);
            }
        });
    if let Err(e) = started {
        log!("can't start a {name} thread: {e}");
        let incoming = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(incoming) = incoming {
            let _ = incoming.stream.end(io_failure(&e));
        }
    }
}

/// A host's error as the reply's status.
pub(crate) fn io_failure(e: &io::Error) -> Status {
    Status::Error(IoError::from(e))
}

/// `path` on this host, which must be absolute: a host path (D § 6), never
/// one the daemon's working directory would resolve.
pub(crate) fn absolute(path: &fe_remote_proto::PathBytes) -> io::Result<PathBuf> {
    let path = crate::convert::path(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(not_absolute(&path))
    }
}

fn not_absolute(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{} isn't an absolute path on this host", path.display()),
    )
}

/// Sends `items` as the bodies `wrap` makes of them: as many to a frame as
/// keep its body under [`CHUNK`], and at most `most`. An item over that on
/// its own goes alone.
pub(crate) fn send_batched<X: Serialize, B: Serialize>(
    stream: &Stream,
    items: impl IntoIterator<Item = X>,
    most: usize,
    wrap: impl Fn(Vec<X>) -> B,
) -> Result<(), MuxError> {
    // The body's tag, and room for the batch's length to grow.
    let base = encoded_len(&wrap(Vec::new()))? + 4;
    let mut batch = Vec::new();
    let mut size = base;
    for item in items {
        let len = encoded_len(&item)?;
        if !batch.is_empty() && (size + len > CHUNK || batch.len() == most) {
            stream.send(&wrap(std::mem::take(&mut batch)))?;
            size = base;
        }
        size += len;
        batch.push(item);
    }
    if !batch.is_empty() {
        stream.send(&wrap(batch))?;
    }
    Ok(())
}

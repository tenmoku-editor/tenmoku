//! The remote development client's connect
//! (docs/decisions/j8wvgszah0-remote-development.md § 1, § 2.5, § 4; the
//! workflow in docs/designs/remote-development.md § 2.2–§ 3).
//!
//! A [`RemoteTransport`] runs a command on the host with its stdio as the
//! link: [`SshTransport`] runs the system `ssh -T`, its prompts going through
//! fe-git's askpass. Over it, a [`Connector`] runs the [`bootstrap`] script,
//! which reports the host and whether this build's `tenmoku-server` is
//! installed, takes an upload when it isn't, and execs it. Then the
//! handshake, whose refusal reinstalls once. Each step is a [`State`] for
//! the connect dialog, and a failure is one of the error table's messages,
//! with the connection log behind Show Log.
//!
//! Where the server comes from is a [`ServerSource`]: a release archive the
//! caller has downloaded and verified, a `remote.dev_server` binary, or, for
//! a development build without one, nowhere.
//!
//! Once connected, a [`RemoteHost`] is a remote window's fe-host
//! `FsHost`, `WatchHost` and `SearchHost` (D § 5), over a [`Conn`]: the
//! connection now, which whoever reconnects the window replaces. A read
//! whose link drops runs once more on the next connection, and a write
//! fails with [`LinkError::Lost`] (D § 7).

pub mod bootstrap;
mod connect;
#[cfg(unix)]
mod remote;
mod source;
#[cfg(unix)]
mod ssh;
#[cfg(all(unix, feature = "test-support"))]
pub mod testing;
mod transport;

pub use bootstrap::{Asset, Probe};
pub use connect::{
    Answer, ConnectError, Connection, Connector, Failed, SignIn, State, server_build,
};
#[cfg(unix)]
pub use remote::{Conn, LinkError, RemoteHost};
pub use source::{Archive, DevServer, Payload, Release, ServerSource, is_dev_build, pack};
#[cfg(unix)]
pub use ssh::{OPTIONS, SshTransport, remote_word};
pub use transport::{Link, RemoteTransport};

//! The transport seam (D § 1): what runs a command on the host with its
//! stdio as the link. `ssh -T` now; `wsl.exe -d` and `docker exec -i` later.

use fe_git::Askpass;
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::process::Child;

pub trait RemoteTransport: Send + Sync {
    /// Runs `argv` on the host with its stdio as the returned pipe. Its
    /// prompts (a passphrase, a password, an unknown host key) go to
    /// `askpass`.
    fn spawn(&self, argv: &[&OsStr], askpass: &Askpass) -> io::Result<Link>;
    /// "ssh neon", for the pill and the log.
    fn describe(&self) -> String;
}

/// The pipe to a command on the host, and the local child behind it.
pub struct Link {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    /// Its stderr, when piped, is the connection log's.
    pub child: Child,
    /// What the transport keeps alive for as long as the link, such as
    /// ssh's askpass session.
    pub keep: Option<Box<dyn Send>>,
}

impl Link {
    /// The protocol crate's link over the same pipe and child, and what
    /// this one kept.
    pub fn into_proto(self) -> (fe_remote_proto::Link, Option<Box<dyn Send>>) {
        let link = fe_remote_proto::Link {
            reader: self.reader,
            writer: self.writer,
            child: Some(self.child),
        };
        (link, self.keep)
    }

    /// Hangs up and waits for the child to exit, as it does once the
    /// command on the host has: its exit status, if it could be read.
    pub fn finish(mut self) -> Option<std::process::ExitStatus> {
        drop(self.writer);
        self.child.wait().ok()
    }

    /// Kills the child and reaps it.
    pub fn abandon(mut self) {
        drop(self.writer);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

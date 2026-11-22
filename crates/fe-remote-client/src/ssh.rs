//! `SshTransport` (D § 1): the system `ssh -T`, so the user's ssh config,
//! agent, `ProxyJump` and `known_hosts` all apply, with its prompts sent
//! through fe-git's askpass.

use crate::transport::{Link, RemoteTransport};
use fe_git::Askpass;
use fe_git::askpass::Session;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::process::CommandExt as _;
use std::process::{Command, Stdio};

/// The options always passed (D § 1): no tty, since the stdio is binary; a
/// keepalive that notices a dead TCP path even when the heartbeat is
/// starved; and a connect timeout. Never `BatchMode`, so prompts reach the
/// user, and no `ControlMaster`, though the user's config may add one.
pub const OPTIONS: [&str; 7] = [
    "-T",
    "-o",
    "ServerAliveInterval=15",
    "-o",
    "ServerAliveCountMax=3",
    "-o",
    "ConnectTimeout=15",
];

/// `ssh -T <options> <destination> -- <argv>`.
pub struct SshTransport {
    destination: String,
    env: Vec<(OsString, OsString)>,
}

impl SshTransport {
    /// For `destination`, an ssh alias or `[user@]host`, as ssh takes it.
    /// One starting with `-` is refused: ssh would read it as an option.
    pub fn new(destination: impl Into<String>) -> io::Result<SshTransport> {
        let destination = destination.into();
        if destination.is_empty() || destination.starts_with('-') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{destination:?} isn't an ssh destination"),
            ));
        }
        Ok(SshTransport {
            destination,
            env: Vec::new(),
        })
    }

    /// Sets `key` in ssh's environment, as `Command::env` does. A `PATH`
    /// set here is also where `ssh` is looked up.
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> SshTransport {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// ssh's arguments for running `argv` on the host. ssh joins the
    /// command's words with spaces for the login shell to split again, so
    /// each word that isn't plain is single-quoted for it.
    pub fn args(&self, argv: &[&OsStr]) -> Vec<OsString> {
        let mut args: Vec<OsString> = OPTIONS.iter().map(OsString::from).collect();
        args.push(self.destination.clone().into());
        args.push("--".into());
        args.extend(argv.iter().map(|word| remote_word(word)));
        args
    }
}

impl RemoteTransport for SshTransport {
    fn spawn(&self, argv: &[&OsStr], askpass: &Askpass) -> io::Result<Link> {
        let mut command = Command::new("ssh");
        command.args(self.args(argv));
        command.envs(self.env.iter().map(|(k, v)| (k, v)));
        spawn_asking(&mut command, askpass, &self.destination)
    }

    fn describe(&self) -> String {
        format!("ssh {}", self.destination)
    }
}

/// Spawns `command` with its stdio piped and its prompts going to
/// `askpass` for `remote`: `SSH_ASKPASS` is the session's script, with
/// `SSH_ASKPASS_REQUIRE=force` so ssh asks through it even with a terminal
/// to read from. The command leads a process group of its own, which a
/// cancelled prompt stops. The session lives as long as the link.
pub(crate) fn spawn_asking(
    command: &mut Command,
    askpass: &Askpass,
    remote: &str,
) -> io::Result<Link> {
    // Without it ssh would prompt on a terminal the app may have, unseen,
    // so a session that can't be set up fails the spawn.
    let session = Session::start(askpass, remote)?;
    command
        .env("SSH_ASKPASS", session.script())
        .env("SSH_ASKPASS_REQUIRE", "force")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn()?;
    session.started(child.id());
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::other("the child's stdio wasn't piped"));
    };
    Ok(Link {
        reader: Box::new(stdout),
        writer: Box::new(stdin),
        child,
        keep: Some(Box::new(session)),
    })
}

/// `word` as a POSIX shell reads it back: as it is when every byte is
/// plain, else single-quoted.
pub fn remote_word(word: &OsStr) -> OsString {
    let bytes = word.as_bytes();
    let plain = |b: &u8| b.is_ascii_alphanumeric() || b"%+,-./:=@_".contains(b);
    if !bytes.is_empty() && bytes.iter().all(plain) {
        return word.to_owned();
    }
    let mut quoted = vec![b'\''];
    for &b in bytes {
        if b == b'\'' {
            quoted.extend(b"'\\''");
        } else {
            quoted.push(b);
        }
    }
    quoted.push(b'\'');
    OsStr::from_bytes(&quoted).to_owned()
}

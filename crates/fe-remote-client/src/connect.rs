//! Connecting (design § 2.2, § 2.4, § 3; D § 2.5, § 4): ssh, the
//! bootstrap's probe, the upload when the host lacks this build's server,
//! and the handshake, with a [`State`] for each step and the error table's
//! messages when one fails.
//!
//! A refused handshake (by either side) on a release install reinstalls
//! once: the server there is another build, say a stale install. A second
//! refusal is an error. A `remote.dev_server` is never reinstalled, as its
//! folder is named for its contents: reinstalling uploads the same binary.

use crate::bootstrap::{self, Asset, Line, Probe};
use crate::source::{DevServer, Payload, ServerSource};
use crate::transport::{Link, RemoteTransport};
use fe_git::askpass::Withdrawn;
use fe_git::{Askpass, AskpassRequest};
use fe_remote_proto::{HandshakeError, Location, Mux, Welcome};
use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead as _, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How much of the archive goes in one write, and so how often
/// [`State::Uploading`] reports.
const UPLOAD_CHUNK: usize = 64 * 1024;

/// The longest line read from the bootstrap.
const MAX_LINE: usize = 4096;

/// How many lines before the probe are let pass: a login shell's greeting.
const MAX_GREETING: usize = 256;

/// The lines the connection log keeps.
const LOG_LINES: usize = 2000;

/// How long a failed ssh's stderr is waited on to end, once ssh has exited:
/// a `ProxyCommand` left running may hold it open.
const STDERR_WAIT: Duration = Duration::from_secs(2);

/// Where a connect is, as the connect dialog's status line shows it
/// (design § 2.2). Downloading and verifying a release archive come before
/// the upload, from the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// ssh is starting: "Connecting to neon…".
    Connecting,
    /// An askpass prompt is up: "Waiting for you to sign in to neon". The
    /// next state ends it.
    SigningIn {
        prompt: String,
    },
    /// The bootstrap's first line: "Checking neon…".
    Probing(Probe),
    /// The archive is going up: "Installing on neon… 60%".
    Uploading {
        sent: u64,
        total: u64,
    },
    /// The handshake was refused, so the server is reinstalled once:
    /// "tenmoku-server on neon is a different build (0.2.9+ab12cd3).
    /// Reinstalling…". `theirs` is the server's build, or the refusal's
    /// reason when it names none.
    Reinstalling {
        theirs: String,
    },
    /// The server is starting, until the handshake's `Welcome`: "Starting
    /// tenmoku-server…".
    Starting,
    Connected,
}

/// Why a connect failed (design § 2.4).
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("Couldn't start {what}: {error}")]
    Spawn { what: String, error: io::Error },
    /// ssh failed: its last stderr line.
    #[error("Couldn't connect to {host}: {line}")]
    Ssh { host: String, line: String },
    #[error("{host} didn't accept the sign-in.")]
    SignIn { host: String },
    #[error(
        "The host key for {host} has changed. This could mean someone is intercepting the \
         connection. Tenmoku won't connect until the key in ~/.ssh/known_hosts is fixed."
    )]
    HostKeyChanged { host: String },
    #[error("tenmoku-server isn't built for {os} {arch}.")]
    Unsupported { os: String, arch: String },
    #[error(
        "This development build has no matching tenmoku-server release. Set \
         remote.dev_server to a server built for {asset}."
    )]
    NoRelease { asset: Asset },
    #[error("Couldn't read the tenmoku-server archive for {asset}: {error}")]
    Archive { asset: Asset, error: io::Error },
    #[error("Couldn't read remote.dev_server {}: {error}", path.display())]
    DevServer { path: PathBuf, error: io::Error },
    #[error("{build:?} can't name a tenmoku-server install")]
    Build { build: String },
    #[error("The bootstrap on {host} printed {line:?}")]
    Bootstrap { host: String, line: String },
    #[error("Couldn't install on {host}: {message}")]
    Install { host: String, message: String },
    /// D § 4's message: a dev server is never reinstalled.
    #[error(
        "remote.dev_server is build {theirs}, this client is {ours}. Build it from the same checkout."
    )]
    DevServerBuild { theirs: String, ours: String },
    /// The handshake was refused again after the reinstall.
    #[error("tenmoku-server on {host} is still a different build ({theirs}) after reinstalling.")]
    StillRefused { host: String, theirs: String },
    #[error("Couldn't start tenmoku-server on {host}: {error}")]
    Handshake { host: String, error: HandshakeError },
}

/// A failed connect: why, and the connection log for Show Log.
#[derive(Debug)]
pub struct Failed {
    pub error: ConnectError,
    pub log: String,
}

/// The answer function a sign-in prompt is put to, as `Askpass::new` takes
/// it.
pub type Answer = Arc<dyn Fn(&AskpassRequest, &Withdrawn) -> Option<String> + Send + Sync>;

/// How ssh's prompts reach the user: `Askpass::new`'s arguments.
#[derive(Clone)]
pub struct SignIn {
    /// The askpass helper and its arguments, such as `tenmoku --askpass`.
    pub helper: Vec<OsString>,
    /// Where each connect's private askpass folder is made.
    pub dir: PathBuf,
    pub answer: Answer,
}

/// What a connect needs.
pub struct Connector {
    pub transport: Arc<dyn RemoteTransport>,
    /// The host as messages name it: `neon`.
    pub host: String,
    pub location: Location,
    pub source: ServerSource,
    /// The handshake's protocol, build and window, and the heartbeat. The
    /// build also names the install.
    pub mux: fe_remote_proto::Config,
    pub sign_in: SignIn,
}

/// A connected link to the host's server.
pub struct Connection {
    mux: Mux,
    welcome: Welcome,
    log: Log,
    /// The transport's, such as ssh's askpass session: after the mux, so
    /// it outlives the link. In a mutex only so a connection can be shared
    /// between threads.
    _keep: Mutex<Option<Box<dyn Send>>>,
}

impl Connection {
    /// A connection over a link made another way than [`Connector::connect`]:
    /// an in-process server's, say. Its log is empty.
    pub fn over(mux: Mux, welcome: Welcome) -> Connection {
        Connection {
            mux,
            welcome,
            log: Log::default(),
            _keep: Mutex::new(None),
        }
    }

    pub fn mux(&self) -> &Mux {
        &self.mux
    }

    pub fn welcome(&self) -> &Welcome {
        &self.welcome
    }

    /// The connection log: ssh's stderr, which keeps coming, and the
    /// bootstrap's transcript.
    pub fn log(&self) -> String {
        self.log.text()
    }

    /// Hangs up.
    pub fn close(self) {
        self.mux.close();
    }
}

/// ssh's stderr and the bootstrap's transcript, as Show Log shows them.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<LogLines>>);

#[derive(Default)]
struct LogLines {
    all: VecDeque<String>,
    /// stderr's alone, to tell why ssh failed.
    stderr: VecDeque<String>,
}

impl Log {
    fn lock(&self) -> std::sync::MutexGuard<'_, LogLines> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn push(&self, line: String, stderr: bool) {
        let mut lines = self.lock();
        let keep = |lines: &mut VecDeque<String>, line: String| {
            if lines.len() == LOG_LINES {
                lines.pop_front();
            }
            lines.push_back(line);
        };
        if stderr {
            keep(&mut lines.stderr, line.clone());
        }
        keep(&mut lines.all, line);
    }

    /// A line of the bootstrap's transcript.
    fn boot(&self, line: impl Into<String>) {
        self.push(format!("bootstrap: {}", line.into()), false);
    }

    fn text(&self) -> String {
        let lines = self.lock();
        let mut text = String::new();
        for line in &lines.all {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    fn stderr(&self) -> Vec<String> {
        self.lock().stderr.iter().cloned().collect()
    }
}

/// What gets installed: the folder, and the payload when it's known before
/// the probe (a dev server's).
struct Target {
    dir: String,
    payload: Option<Payload>,
}

type States = Arc<dyn Fn(&State) + Send + Sync>;

impl Connector {
    /// Connects, reporting each [`State`] to `on_state` as it's reached,
    /// from this thread or, for [`State::SigningIn`], the askpass's.
    /// Blocks until connected or failed.
    pub fn connect(
        &self,
        on_state: impl Fn(&State) + Send + Sync + 'static,
    ) -> Result<Connection, Failed> {
        let on_state: States = Arc::new(on_state);
        let cancelled = Arc::new(AtomicBool::new(false));
        let askpass = {
            let (answer, on_state, cancelled) = (
                self.sign_in.answer.clone(),
                on_state.clone(),
                cancelled.clone(),
            );
            Askpass::new(
                self.sign_in.helper.clone(),
                self.sign_in.dir.clone(),
                move |request, withdrawn| {
                    on_state(&State::SigningIn {
                        prompt: request.prompt.clone(),
                    });
                    let answered = answer(request, withdrawn);
                    if answered.is_none() && !withdrawn.is_withdrawn() {
                        cancelled.store(true, Ordering::SeqCst);
                    }
                    answered
                },
            )
        };
        let connect = Connect {
            connector: self,
            log: Log::default(),
            on_state,
            askpass,
            cancelled,
        };
        connect.run().map_err(|error| Failed {
            error,
            log: connect.log.text(),
        })
    }
}

/// One connect's state.
struct Connect<'a> {
    connector: &'a Connector,
    log: Log,
    on_state: States,
    askpass: Askpass,
    /// The user cancelled a sign-in prompt.
    cancelled: Arc<AtomicBool>,
}

/// A handshake refused by either side, which a reinstall may fix.
struct Refusal {
    theirs: String,
}

impl Connect<'_> {
    fn run(&self) -> Result<Connection, ConnectError> {
        let c = self.connector;
        let build = &c.mux.build;
        let target = match &c.source {
            ServerSource::Dev(path) => {
                let dev = DevServer::read(path).map_err(|error| ConnectError::DevServer {
                    path: path.clone(),
                    error,
                })?;
                Target {
                    dir: dev.dir,
                    payload: Some(dev.payload),
                }
            }
            ServerSource::Release(_) | ServerSource::NoRelease => Target {
                dir: build.clone(),
                payload: None,
            },
        };
        let mut reinstall = false;
        loop {
            let refusal = match self.attempt(&target, reinstall)? {
                Ok(connection) => return Ok(connection),
                Err(refusal) => refusal,
            };
            self.log
                .boot(format!("the handshake was refused: {}", refusal.theirs));
            let theirs = refusal.theirs;
            if matches!(c.source, ServerSource::Dev(_)) {
                return Err(ConnectError::DevServerBuild {
                    theirs,
                    ours: build.clone(),
                });
            }
            if reinstall {
                return Err(ConnectError::StillRefused {
                    host: c.host.clone(),
                    theirs,
                });
            }
            self.state(State::Reinstalling { theirs });
            reinstall = true;
        }
    }

    fn state(&self, state: State) {
        (self.on_state)(&state);
    }

    /// One ssh session: probe, install if need be, and the handshake.
    fn attempt(
        &self,
        target: &Target,
        reinstall: bool,
    ) -> Result<Result<Connection, Refusal>, ConnectError> {
        let c = self.connector;
        let script =
            bootstrap::script(&target.dir, reinstall).ok_or_else(|| ConnectError::Build {
                build: c.mux.build.clone(),
            })?;
        self.state(State::Connecting);
        self.log.boot(format!(
            "{} (installing to {}{})",
            c.transport.describe(),
            target.dir,
            if reinstall { ", reinstalling" } else { "" }
        ));
        let argv: [&OsStr; 3] = ["sh".as_ref(), "-c".as_ref(), script.as_ref()];
        let mut link =
            (c.transport.spawn(&argv, &self.askpass)).map_err(|error| ConnectError::Spawn {
                what: c.transport.describe(),
                error,
            })?;
        let stderr_done = self.read_stderr(&mut link);
        let outcome = self.drive(link, target, reinstall, &stderr_done);
        if !matches!(outcome, Ok(Ok(_))) {
            // The child is gone: its last words belong in the log.
            let _ = stderr_done.recv_timeout(STDERR_WAIT);
        }
        outcome
    }

    /// The rest of [`Connect::attempt`], once the link is up.
    fn drive(
        &self,
        mut link: Link,
        target: &Target,
        reinstall: bool,
        stderr_done: &mpsc::Receiver<()>,
    ) -> Result<Result<Connection, Refusal>, ConnectError> {
        let c = self.connector;
        let host = || c.host.clone();
        let probe = match self.probe(&mut link.reader) {
            Ok(Some(probe)) => probe,
            Ok(None) => return Err(self.ssh_failed(link, stderr_done)),
            Err(line) => {
                link.abandon();
                return Err(ConnectError::Bootstrap { host: host(), line });
            }
        };
        self.state(State::Probing(probe.clone()));
        let Some(asset) = Asset::for_host(&probe.os, &probe.arch) else {
            link.abandon();
            return Err(ConnectError::Unsupported {
                os: probe.os,
                arch: probe.arch,
            });
        };
        if matches!(c.source, ServerSource::NoRelease) {
            // Even with a server there: another checkout's build can have
            // the same id.
            link.abandon();
            return Err(ConnectError::NoRelease { asset });
        }

        let mut sent = Ok(());
        if !probe.present || reinstall {
            let payload = match (&c.source, &target.payload) {
                (_, Some(payload)) => payload.clone(),
                (ServerSource::Release(release), None) => {
                    match release(asset).and_then(|archive| Payload::archive(&archive)) {
                        Ok(payload) => payload,
                        Err(error) => {
                            link.abandon();
                            return Err(ConnectError::Archive { asset, error });
                        }
                    }
                }
                (_, None) => {
                    link.abandon();
                    return Err(ConnectError::NoRelease { asset });
                }
            };
            sent = self.upload(&mut link.writer, &payload);
        }

        match read_line(&mut link.reader).map(|line| line.map(|l| (Line::parse(&l), l))) {
            Ok(Some((Some(Ok(Line::Ready)), line))) => self.log.boot(line),
            Ok(Some((Some(Ok(Line::Error(message))), line))) => {
                self.log.boot(line);
                link.finish();
                return Err(ConnectError::Install {
                    host: host(),
                    message,
                });
            }
            Ok(Some((_, line))) => {
                link.abandon();
                return Err(ConnectError::Bootstrap { host: host(), line });
            }
            Ok(None) | Err(_) => {
                link.finish();
                let _ = stderr_done.recv_timeout(STDERR_WAIT);
                let message = match (sent, self.log.stderr().pop()) {
                    (Err(e), _) => format!("the upload failed: {e}"),
                    (Ok(()), Some(line)) => line,
                    (Ok(()), None) => "the bootstrap ended without starting the server".into(),
                };
                return Err(ConnectError::Install {
                    host: host(),
                    message,
                });
            }
        }

        self.state(State::Starting);
        let (link, keep) = link.into_proto();
        match Mux::client(link, c.mux.clone(), c.location.clone(), None) {
            Ok((mux, welcome)) => {
                self.log
                    .boot(format!("connected to build {}", welcome.build));
                self.state(State::Connected);
                Ok(Ok(Connection {
                    mux,
                    welcome,
                    log: self.log.clone(),
                    _keep: Mutex::new(keep),
                }))
            }
            Err(HandshakeError::Mismatch { build, .. }) => Ok(Err(Refusal { theirs: build })),
            Err(HandshakeError::Refused(reason)) => Ok(Err(Refusal {
                theirs: server_build(&reason).unwrap_or(reason),
            })),
            Err(error) => Err(ConnectError::Handshake {
                host: host(),
                error,
            }),
        }
    }

    /// Copies the child's stderr, if piped, into the log on a thread of its
    /// own; the receiver hears when it ends.
    fn read_stderr(&self, link: &mut Link) -> mpsc::Receiver<()> {
        let (done, ended) = mpsc::channel();
        if let Some(stderr) = link.child.stderr.take() {
            let log = self.log.clone();
            let copy = move || {
                // Bytes, not `lines`: a path in a message needn't be UTF-8,
                // and the lines after it still count.
                let mut stderr = BufReader::new(stderr);
                let mut line = Vec::new();
                while matches!(stderr.read_until(b'\n', &mut line), Ok(1..)) {
                    let text = String::from_utf8_lossy(&line);
                    log.push(text.trim_end_matches(['\n', '\r']).to_owned(), true);
                    line.clear();
                }
                let _ = done.send(());
            };
            // One that can't start leaves the log without stderr, and
            // nothing to wait for.
            let _ = std::thread::Builder::new()
                .name("remote-ssh-stderr".into())
                .spawn(copy);
        }
        ended
    }

    /// The probe line, after any greeting: `Ok(None)` if the output ended
    /// first, and `Err` with the line if the script said something else.
    fn probe(&self, reader: &mut dyn Read) -> Result<Option<Probe>, String> {
        for _ in 0..MAX_GREETING {
            let line = match read_line(reader) {
                Ok(Some(line)) => line,
                Ok(None) | Err(_) => return Ok(None),
            };
            match Line::parse(&line) {
                Some(Ok(Line::Probe(probe))) => {
                    self.log.boot(line);
                    return Ok(Some(probe));
                }
                Some(Ok(_) | Err(_)) => return Err(line),
                None => self.log.push(format!("host: {line}"), false),
            }
        }
        Err("the host printed too much before the bootstrap".into())
    }

    /// Sends `<size> <sha256>` and the archive, reporting progress. Stops
    /// at the first failed write: the script has ended, and says why.
    fn upload(&self, writer: &mut dyn Write, payload: &Payload) -> io::Result<()> {
        let total = payload.bytes.len() as u64;
        self.log
            .boot(format!("sending {total} bytes, SHA-256 {}", payload.sha256));
        self.state(State::Uploading { sent: 0, total });
        writer.write_all(format!("{total} {}\n", payload.sha256).as_bytes())?;
        let mut sent = 0;
        for chunk in payload.bytes.chunks(UPLOAD_CHUNK) {
            writer.write_all(chunk)?;
            sent += chunk.len() as u64;
            self.state(State::Uploading { sent, total });
        }
        writer.flush()
    }

    /// Why ssh ended before the bootstrap spoke (design § 2.4).
    fn ssh_failed(&self, link: Link, stderr_done: &mpsc::Receiver<()>) -> ConnectError {
        let status = link.finish();
        let _ = stderr_done.recv_timeout(STDERR_WAIT);
        let host = self.connector.host.clone();
        let stderr = self.log.stderr();
        let said = |text: &str| stderr.iter().any(|line| line.contains(text));
        if self.cancelled.load(Ordering::SeqCst) {
            return ConnectError::SignIn { host };
        }
        if said("REMOTE HOST IDENTIFICATION HAS CHANGED") {
            return ConnectError::HostKeyChanged { host };
        }
        if said("Permission denied (")
            || said("Too many authentication failures")
            || said("Host key verification failed")
        {
            return ConnectError::SignIn { host };
        }
        let line = (stderr.iter().rev())
            .find(|line| !line.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| exited(&self.connector.transport.describe(), status));
        ConnectError::Ssh { host, line }
    }
}

fn exited(what: &str, status: Option<ExitStatus>) -> String {
    match status {
        Some(status) => format!("{what} ended ({status})"),
        None => format!("{what} ended"),
    }
}

/// The server's build in a refusal's reason, as fe-remote-proto words it:
/// `client protocol 1 build "A", server protocol 1 build "B"`.
pub fn server_build(reason: &str) -> Option<String> {
    let server = &reason[reason.find("server protocol ")?..];
    let build = &server[server.find("build \"")? + "build \"".len()..];
    Some(build[..build.find('"')?].to_owned())
}

/// One line, read a byte at a time so nothing past it is taken from the
/// pipe: the rest is the protocol's. `None` at the end.
fn read_line(reader: &mut dyn Read) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    let mut byte = [0];
    while line.len() < MAX_LINE {
        match reader.read(&mut byte) {
            Ok(0) if line.is_empty() => return Ok(None),
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => line.push(byte[0]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let line = String::from_utf8_lossy(&line);
    Ok(Some(line.trim_end_matches('\r').to_owned()))
}

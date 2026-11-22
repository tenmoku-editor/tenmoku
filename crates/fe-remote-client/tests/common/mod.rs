//! What the connect tests share: a host that is this machine with a
//! temporary HOME and XDG_CACHE_HOME, the fixture binary, and a recorder of
//! the states a connect reports.

#![allow(dead_code, unreachable_pub)]

pub mod fake_ssh;

use fe_remote_client::testing::{self, ChildTransport};
use fe_remote_client::{Answer, Archive, Connector, Probe, Release, ServerSource, State};
use fe_remote_proto::BUILD;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

pub fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fe-remote-client-fixture"))
}

/// `uname -s` and `uname -m` here.
pub fn uname() -> (String, String) {
    let word = |flag| {
        let out = Command::new("uname").arg(flag).output().unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    (word("-s"), word("-m"))
}

/// The probe line this machine gives.
pub fn probe(present: bool) -> Probe {
    let (os, arch) = uname();
    Probe { os, arch, present }
}

pub struct Host {
    pub dir: tempfile::TempDir,
}

impl Host {
    pub fn new() -> Host {
        let host = Host {
            // Under /tmp, not TMPDIR: a long TMPDIR (macOS's) would push the
            // daemon's socket past SOCKET_PATH_MAX and the lock files onto
            // the shared /tmp/tenmoku-server-<uid>/ fallback.
            dir: tempfile::Builder::new().tempdir_in("/tmp").unwrap(),
        };
        for dir in [host.home(), host.cache(), host.askpass_dir()] {
            std::fs::create_dir_all(dir).unwrap();
        }
        host
    }

    /// `name` made unique to this host: the daemon lock the script looks
    /// for is named by build, and a build's name must not meet another
    /// test's (or an earlier run's) lock.
    pub fn unique(&self, name: &str) -> String {
        format!("{name}-{}", self.tag())
    }

    /// Strips what `unique` added.
    pub fn plain<'a>(&self, name: &'a str) -> &'a str {
        name.strip_suffix(&format!("-{}", self.tag()))
            .unwrap_or(name)
    }

    fn tag(&self) -> String {
        let name = self.dir.path().file_name().unwrap().to_string_lossy();
        name.trim_start_matches('.').to_string()
    }

    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    /// `XDG_CACHE_HOME`, apart from HOME's `.cache` to show it's honoured.
    pub fn cache(&self) -> PathBuf {
        self.dir.path().join("cache")
    }

    pub fn askpass_dir(&self) -> PathBuf {
        self.dir.path().join("askpass")
    }

    /// Where `dir`'s server is installed.
    pub fn server(&self, dir: &str) -> PathBuf {
        self.cache()
            .join("tenmoku-server")
            .join(dir)
            .join("tenmoku-server")
    }

    /// Puts `contents` in place as `dir`'s server.
    pub fn install(&self, dir: &str, contents: &[u8]) {
        let server = self.server(dir);
        std::fs::create_dir_all(server.parent().unwrap()).unwrap();
        std::fs::write(&server, contents).unwrap();
        set_mode(&server, 0o755);
    }

    pub fn env(&self) -> Vec<(OsString, OsString)> {
        vec![
            ("HOME".into(), self.home().into()),
            ("XDG_CACHE_HOME".into(), self.cache().into()),
        ]
    }

    pub fn transport(&self, prelude: Vec<OsString>) -> Arc<ChildTransport> {
        Arc::new(ChildTransport {
            prelude,
            env: self.env(),
        })
    }

    /// A connector over `transport` as this build, whose prompts get
    /// `answer`.
    pub fn connector(
        &self,
        transport: Arc<ChildTransport>,
        source: ServerSource,
        answer: Answer,
    ) -> Connector {
        self.connector_as(BUILD, transport, source, answer)
    }

    pub fn connector_as(
        &self,
        build: &str,
        transport: Arc<ChildTransport>,
        source: ServerSource,
        answer: Answer,
    ) -> Connector {
        Connector {
            transport,
            host: "test".into(),
            location: fe_remote_proto::testing::location(),
            source,
            mux: fe_remote_proto::Config {
                build: build.into(),
                heartbeat: None,
                ..fe_remote_proto::Config::default()
            },
            sign_in: testing::sign_in(&fixture(), self.askpass_dir(), answer),
        }
    }
}

pub fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// Answers every prompt with `answer`.
pub fn answering(answer: &str) -> Answer {
    let answer = answer.to_owned();
    Arc::new(move |_, _| Some(answer.clone()))
}

/// A prompt nobody expects.
pub fn no_prompts() -> Answer {
    Arc::new(|request, _| panic!("unexpected prompt {request:?}"))
}

/// The fake server answering as `build`, packed as a release archive, with
/// `padding` incompressible bytes beside it so the upload takes several
/// chunks.
pub fn server_archive(build: &str, args: &[&str], padding: usize) -> Vec<u8> {
    let server = testing::server_script(&fixture(), build, args);
    let padding = noise(padding);
    testing::archive(&[
        ("tenmoku-server", &server, 0o644),
        ("padding", &padding, 0o644),
    ])
}

/// A release source that serves `archive` for every asset.
pub fn release(archive: Vec<u8>) -> ServerSource {
    let archive: Arc<[u8]> = archive.into();
    let release: Release = Arc::new(move |_| Ok(Archive::Bytes(archive.clone())));
    ServerSource::Release(release)
}

/// Bytes no compressor shrinks: xorshift.
pub fn noise(len: usize) -> Vec<u8> {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// Collects the states a connect reports.
#[derive(Clone, Default)]
pub struct States(Arc<Mutex<Vec<State>>>);

impl States {
    pub fn recorder(&self) -> impl Fn(&State) + Send + Sync + 'static {
        let states = self.0.clone();
        move |state| states.lock().unwrap().push(state.clone())
    }

    pub fn take(&self) -> Vec<State> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// The upload's states for `total` bytes, in 64 KiB chunks.
pub fn uploading(total: u64) -> Vec<State> {
    let mut states = vec![State::Uploading { sent: 0, total }];
    let mut sent = 0;
    while sent < total {
        sent = (sent + 64 * 1024).min(total);
        states.push(State::Uploading { sent, total });
    }
    states
}

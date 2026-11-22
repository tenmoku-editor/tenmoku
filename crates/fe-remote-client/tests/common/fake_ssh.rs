//! A fake `ssh` on PATH that records its argv and its askpass environment,
//! then plays a failure or a prompt as `FAKE_SSH` says.

use super::Host;
use fe_remote_client::SshTransport;
use std::ffi::OsString;
use std::path::PathBuf;

const SCRIPT: &str = r#"#!/bin/sh
for a in "$@"; do printf '%s\0' "$a"; done >"$FAKE_SSH_RECORD/argv"
printf '%s\n' "${SSH_ASKPASS-}" "${SSH_ASKPASS_REQUIRE-}" >"$FAKE_SSH_RECORD/env"
case $FAKE_SSH in
refused)
  echo "ssh: connect to host neon port 22: Connection refused" >&2
  exit 255;;
ask)
  "$SSH_ASKPASS" "Enter passphrase for key '/home/me/.ssh/id_ed25519': " >"$FAKE_SSH_RECORD/answer" ||
    { echo "me@neon: Permission denied (publickey)." >&2; exit 255; }
  exit 0;;
hostkey)
  echo "@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@" >&2
  echo "@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @" >&2
  echo "Host key verification failed." >&2
  exit 255;;
esac
"#;

pub struct FakeSsh {
    pub record: PathBuf,
    bin: PathBuf,
}

impl FakeSsh {
    pub fn new(host: &Host) -> FakeSsh {
        let bin = host.dir.path().join("fake-ssh");
        let record = host.dir.path().join("record");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&record).unwrap();
        let ssh = bin.join("ssh");
        std::fs::write(&ssh, SCRIPT).unwrap();
        super::set_mode(&ssh, 0o755);
        FakeSsh { record, bin }
    }

    /// An `SshTransport` to `neon` that finds this ssh first on PATH and
    /// plays `mode`.
    pub fn transport(&self, mode: &str) -> SshTransport {
        let mut path = OsString::from(&self.bin);
        path.push(":");
        path.push(std::env::var_os("PATH").unwrap());
        SshTransport::new("neon")
            .unwrap()
            .env("PATH", path)
            .env("FAKE_SSH_RECORD", &self.record)
            .env("FAKE_SSH", mode)
    }

    pub fn argv(&self) -> Vec<String> {
        let argv = std::fs::read(self.record.join("argv")).unwrap();
        let argv = String::from_utf8(argv).unwrap();
        let mut words: Vec<String> = argv.split('\0').map(str::to_owned).collect();
        words.pop();
        words
    }

    /// `SSH_ASKPASS` and `SSH_ASKPASS_REQUIRE` as ssh saw them.
    pub fn askpass_env(&self) -> (String, String) {
        let env = std::fs::read_to_string(self.record.join("env")).unwrap();
        let mut lines = env.lines().map(str::to_owned);
        (lines.next().unwrap(), lines.next().unwrap())
    }

    pub fn answer(&self) -> String {
        std::fs::read_to_string(self.record.join("answer")).unwrap()
    }
}

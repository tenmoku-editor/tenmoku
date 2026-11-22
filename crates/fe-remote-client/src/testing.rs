//! For tests: a child-stdio transport, and archives holding a fake server.

use crate::connect::{Answer, SignIn};
use crate::transport::{Link, RemoteTransport};
use fe_git::Askpass;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs the bootstrap's argv here, after `prelude`, with `env` set: the
/// host is this machine and the "ssh" is `prelude`. Prompts go to the
/// askpass as ssh's do (`SSH_ASKPASS`), so a prelude that runs
/// `"$SSH_ASKPASS" <prompt>` plays ssh asking for a password.
pub struct ChildTransport {
    pub prelude: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
}

impl RemoteTransport for ChildTransport {
    fn spawn(&self, argv: &[&OsStr], askpass: &Askpass) -> io::Result<Link> {
        let (program, args) = self
            .prelude
            .split_first()
            .ok_or_else(|| io::Error::other("an empty prelude"))?;
        let mut command = Command::new(program);
        command.args(args).args(argv);
        command.envs(self.env.iter().map(|(k, v)| (k, v)));
        crate::ssh::spawn_asking(&mut command, askpass, "test")
    }

    fn describe(&self) -> String {
        "child test".into()
    }
}

/// A prelude that runs `"$SSH_ASKPASS" "<prompt>"`, as ssh would to sign
/// in, and goes on only when it's answered.
pub fn asking_prelude(prompt: &str) -> Vec<OsString> {
    let script = format!("\"$SSH_ASKPASS\" '{prompt}' >/dev/null && exec \"$@\"");
    ["sh", "-c", &script, "sh"].map(OsString::from).to_vec()
}

/// A prelude that runs the argv as it is.
pub fn plain_prelude() -> Vec<OsString> {
    vec!["env".into()]
}

/// A sign-in whose prompts `fixture askpass` sends to `answer`, with the
/// askpass folders made in `dir`.
pub fn sign_in(fixture: &Path, dir: PathBuf, answer: Answer) -> SignIn {
    SignIn {
        helper: vec![fixture.into(), "askpass".into()],
        dir,
        answer,
    }
}

/// A `.tar.gz` of `files`, each a name, its contents and its mode.
pub fn archive(files: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for &(name, data, mode) in files {
        let mut header = tar::Header::new_ustar();
        header.set_size(data.len() as u64);
        header.set_mode(mode);
        header.set_entry_type(tar::EntryType::Regular);
        tar.append_data(&mut header, name, data).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// A `tenmoku-server` that is `fixture server` answering as `build`, with
/// `args` after it.
pub fn server_script(fixture: &Path, build: &str, args: &[&str]) -> Vec<u8> {
    let mut script = format!(
        "#!/bin/sh\nexec '{}' server --build '{build}'",
        fixture.display()
    );
    for arg in args {
        script.push_str(&format!(" '{arg}'"));
    }
    script.push_str(" \"$@\"\n");
    script.into_bytes()
}

//! `SshTransport` (D § 1) against a recording fake `ssh` on PATH: the
//! options it always passes and never passes, the bootstrap as the remote
//! command, and the prompts going to fe-git's askpass.

#![cfg(unix)]

mod common;

use common::fake_ssh::FakeSsh;
use common::{Host, States, fixture, release};
use fe_git::{Askpass, AskpassRequest};
use fe_remote_client::{ConnectError, Connector, RemoteTransport, SshTransport, bootstrap};
use fe_remote_proto::BUILD;
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};

fn connector(host: &Host, transport: SshTransport) -> Connector {
    let mut connector = host.connector(
        host.transport(Vec::new()),
        release(Vec::new()),
        common::answering("hunter2"),
    );
    connector.transport = Arc::new(transport);
    connector.host = "neon".into();
    connector
}

#[test]
fn ssh_runs_the_bootstrap_with_the_fixed_options() {
    let host = Host::new();
    let ssh = FakeSsh::new(&host);
    let failed = connector(&host, ssh.transport("refused"))
        .connect(States::default().recorder())
        .err()
        .unwrap();
    let script = bootstrap::script(BUILD, false).unwrap();
    let expected = [
        "-T",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        "ConnectTimeout=15",
        "neon",
        "--",
        "sh",
        "-c",
        &format!("'{script}'"),
    ];
    assert_eq!(ssh.argv(), expected);
    for arg in ssh.argv() {
        assert!(!arg.contains("BatchMode"), "{arg}");
        assert!(!arg.contains("ControlMaster"), "{arg}");
    }
    assert_eq!(
        failed.error.to_string(),
        "Couldn't connect to neon: ssh: connect to host neon port 22: Connection refused"
    );
    assert!(failed.log.contains("Connection refused"), "{}", failed.log);
}

#[test]
fn ssh_prompts_go_to_the_askpass_with_require_force() {
    let host = Host::new();
    let ssh = FakeSsh::new(&host);
    let asked = Arc::new(Mutex::new(Vec::<AskpassRequest>::new()));
    let seen = asked.clone();
    let askpass = Askpass::new(
        vec![fixture().into(), "askpass".into()],
        host.askpass_dir(),
        move |request, _| {
            seen.lock().unwrap().push(request.clone());
            Some("hunter2".into())
        },
    );
    let transport = ssh.transport("ask");
    let mut link = transport.spawn(&[OsStr::new("true")], &askpass).unwrap();
    assert!(link.child.wait().unwrap().success());

    let (script, require) = ssh.askpass_env();
    assert_eq!(require, "force");
    assert!(
        std::path::Path::new(&script).starts_with(host.askpass_dir()),
        "{script}"
    );
    assert_eq!(ssh.answer(), "hunter2\n");
    let asked = asked.lock().unwrap();
    assert_eq!(
        *asked,
        [AskpassRequest {
            remote: "neon".into(),
            prompt: "Enter passphrase for key '/home/me/.ssh/id_ed25519': ".into(),
        }]
    );
    drop(link);
    assert_eq!(
        std::fs::read_dir(host.askpass_dir()).unwrap().count(),
        0,
        "the askpass folder goes with the link"
    );
}

#[test]
fn ssh_describes_itself_and_refuses_an_option_for_a_destination() {
    let ssh = SshTransport::new("me@neon").unwrap();
    assert_eq!(ssh.describe(), "ssh me@neon");
    assert!(SshTransport::new("-oProxyCommand=evil").is_err());
    assert!(SshTransport::new("").is_err());
}

#[test]
fn remote_words_read_back_as_the_argv_in_sh() {
    let words = ["plain", "two words", "it's", "$HOME", ""];
    let joined: Vec<String> = words
        .iter()
        .map(|w| {
            fe_remote_client::remote_word(OsStr::new(w))
                .into_string()
                .unwrap()
        })
        .collect();
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("printf '%s|' {}", joined.join(" ")))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "plain|two words|it's|$HOME||"
    );
}

#[test]
fn a_cancelled_sign_in_is_not_accepted() {
    let host = Host::new();
    let ssh = FakeSsh::new(&host);
    let mut connector = connector(&host, ssh.transport("ask"));
    connector.sign_in.answer = Arc::new(|_, _| None);
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    assert!(
        matches!(failed.error, ConnectError::SignIn { .. }),
        "{:?}",
        failed.error
    );
    assert_eq!(failed.error.to_string(), "neon didn't accept the sign-in.");
}

#[test]
fn a_changed_host_key_is_its_own_error() {
    let host = Host::new();
    let ssh = FakeSsh::new(&host);
    let failed = connector(&host, ssh.transport("hostkey"))
        .connect(States::default().recorder())
        .err()
        .unwrap();
    assert!(
        matches!(failed.error, ConnectError::HostKeyChanged { .. }),
        "{:?}",
        failed.error
    );
    assert!(
        failed
            .error
            .to_string()
            .starts_with("The host key for neon has changed.")
    );
}

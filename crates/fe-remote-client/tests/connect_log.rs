//! The connection log behind Show Log (design § 2.4): ssh's stderr, all of
//! it, and the bootstrap's transcript, whatever ended the connect.

#![cfg(unix)]

mod common;

use common::{Host, States, no_prompts, release, server_archive};
use fe_remote_client::ConnectError;
use fe_remote_client::testing::{plain_prelude, server_script};
use fe_remote_proto::BUILD;
use std::ffi::OsString;

#[test]
fn stderr_that_isnt_utf8_keeps_the_lines_after_it() {
    let host = Host::new();
    // An ssh that fails, naming a path that isn't UTF-8 first.
    let prelude = [
        "sh",
        "-c",
        "printf 'no such key /home/\\377\\376\\n' >&2; echo 'ssh: the last word' >&2; exit 255",
        "sh",
    ]
    .map(OsString::from)
    .to_vec();
    let connector = host.connector(
        host.transport(prelude),
        release(server_archive(BUILD, &[], 0)),
        no_prompts(),
    );
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    assert_eq!(
        failed.error.to_string(),
        "Couldn't connect to test: ssh: the last word"
    );
    assert!(
        failed.log.contains("no such key /home/\u{fffd}\u{fffd}"),
        "{}",
        failed.log
    );
    assert!(failed.log.contains("ssh: the last word"), "{}", failed.log);
}

#[test]
fn a_server_that_fails_to_start_leaves_its_words_in_the_log() {
    let host = Host::new();
    let server = server_script(
        &common::fixture(),
        BUILD,
        &["--fail-with", "cannot reach the daemon: no space left"],
    );
    host.install(BUILD, &server);
    let connector = host.connector(
        host.transport(plain_prelude()),
        release(Vec::new()),
        no_prompts(),
    );
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    assert!(
        matches!(failed.error, ConnectError::Handshake { .. }),
        "{:?}",
        failed.error
    );
    assert!(
        failed
            .log
            .contains("cannot reach the daemon: no space left"),
        "{}",
        failed.log
    );
    assert!(failed.log.contains("TENMOKU-BOOT ready"), "{}", failed.log);
}

#[test]
fn a_failed_install_leaves_the_script_and_ssh_words_in_the_log() {
    let host = Host::new();
    // Not writable: the install's mkdir fails.
    std::fs::create_dir_all(host.cache().join("tenmoku-server")).unwrap();
    common::set_mode(&host.cache().join("tenmoku-server"), 0o500);
    let connector = host.connector(
        host.transport(plain_prelude()),
        release(server_archive(BUILD, &[], 0)),
        no_prompts(),
    );
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    common::set_mode(&host.cache().join("tenmoku-server"), 0o700);
    assert!(
        matches!(failed.error, ConnectError::Install { .. }),
        "{:?}",
        failed.error
    );
    assert!(
        failed
            .error
            .to_string()
            .starts_with("Couldn't install on test: mkdir"),
        "{}",
        failed.error
    );
    assert!(failed.log.contains("TENMOKU-BOOT error"), "{}", failed.log);
}

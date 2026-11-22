//! A connect's states, in order (design § 2.2), over a child-stdio transport
//! running the bootstrap here and a fake server; and the reinstall a refused
//! handshake makes, once (D § 2.5, design § 2.4).

#![cfg(unix)]

mod common;

use common::{Host, States, answering, no_prompts, probe, release, server_archive, uploading};
use fe_remote_client::testing::{asking_prelude, plain_prelude, server_script};
use fe_remote_client::{ConnectError, State};
use fe_remote_proto::BUILD;

#[test]
fn a_first_connect_reports_each_state_in_order() {
    let host = Host::new();
    let archive = server_archive(BUILD, &[], 200 * 1024);
    let total = archive.len() as u64;
    let connector = host.connector(
        host.transport(asking_prelude("Password:")),
        release(archive),
        answering("hunter2"),
    );
    let states = States::default();
    let connection = connector.connect(states.recorder()).unwrap();
    assert_eq!(connection.welcome().build, BUILD);

    let mut expected = vec![
        State::Connecting,
        State::SigningIn {
            prompt: "Password:".into(),
        },
        State::Probing(probe(false)),
    ];
    expected.extend(uploading(total));
    expected.extend([State::Starting, State::Connected]);
    assert_eq!(states.take(), expected);
    assert!(total > 3 * 64 * 1024, "the upload is several chunks");
    connection.close();
}

#[test]
fn a_connect_to_a_host_with_the_server_skips_the_upload() {
    let host = Host::new();
    host.install(BUILD, &server_script(&common::fixture(), BUILD, &[]));
    let connector = host.connector(
        host.transport(plain_prelude()),
        release(Vec::new()),
        no_prompts(),
    );
    let states = States::default();
    let connection = connector.connect(states.recorder()).unwrap();
    assert_eq!(
        states.take(),
        [
            State::Connecting,
            State::Probing(probe(true)),
            State::Starting,
            State::Connected
        ]
    );
    connection.close();
}

#[test]
fn a_refused_handshake_reinstalls_once() {
    let host = Host::new();
    host.install(
        BUILD,
        &server_script(&common::fixture(), "9.9.9+other", &[]),
    );
    let archive = server_archive(BUILD, &[], 0);
    let total = archive.len() as u64;
    let connector = host.connector(
        host.transport(plain_prelude()),
        release(archive),
        no_prompts(),
    );
    let states = States::default();
    let connection = connector.connect(states.recorder()).unwrap();

    let mut expected = vec![
        State::Connecting,
        State::Probing(probe(true)),
        State::Starting,
        State::Reinstalling {
            theirs: "9.9.9+other".into(),
        },
        State::Connecting,
        State::Probing(probe(true)),
    ];
    expected.extend(uploading(total));
    expected.extend([State::Starting, State::Connected]);
    assert_eq!(states.take(), expected);
    assert_eq!(connection.welcome().build, BUILD);
    connection.close();
}

#[test]
fn a_server_answering_as_another_build_reinstalls_too() {
    let host = Host::new();
    let stale = server_script(&common::fixture(), "9.9.9+other", &["--answer-any"]);
    host.install(BUILD, &stale);
    let connector = host.connector(
        host.transport(plain_prelude()),
        release(server_archive(BUILD, &[], 0)),
        no_prompts(),
    );
    let states = States::default();
    let connection = connector.connect(states.recorder()).unwrap();
    let reinstalls: Vec<State> = (states.take().into_iter())
        .filter(|state| matches!(state, State::Reinstalling { .. }))
        .collect();
    assert_eq!(
        reinstalls,
        [State::Reinstalling {
            theirs: "9.9.9+other".into()
        }]
    );
    connection.close();
}

#[test]
fn a_second_refusal_is_an_error() {
    let host = Host::new();
    host.install(
        BUILD,
        &server_script(&common::fixture(), "9.9.9+other", &[]),
    );
    let connector = host.connector(
        host.transport(plain_prelude()),
        release(server_archive("9.9.9+other", &[], 0)),
        no_prompts(),
    );
    let states = States::default();
    let failed = connector.connect(states.recorder()).err().unwrap();
    assert!(
        matches!(failed.error, ConnectError::StillRefused { .. }),
        "{:?}",
        failed.error
    );
    assert_eq!(
        failed.error.to_string(),
        "tenmoku-server on test is still a different build (9.9.9+other) after reinstalling."
    );
    let states = states.take();
    let reinstalls = states
        .iter()
        .filter(|state| matches!(state, State::Reinstalling { .. }))
        .count();
    assert_eq!(reinstalls, 1, "{states:?}");
    assert!(failed.log.contains("TENMOKU-BOOT ready"), "{}", failed.log);
}

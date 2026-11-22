//! Development builds (D § 4): a `remote.dev_server` binary installs to
//! `dev-<sha256[..12]>/` and must be the client's own build; without one, a
//! development build can't install.

#![cfg(unix)]

mod common;

use common::{Host, States, fixture, no_prompts, probe, set_mode, uploading};
use fe_remote_client::testing::{plain_prelude, server_script};
use fe_remote_client::{Archive, ConnectError, Release, ServerSource, State, pack};
use fe_remote_proto::BUILD;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A `remote.dev_server` that is the fake server answering as `build`.
fn dev_server(host: &Host, build: &str) -> (PathBuf, Vec<u8>) {
    let binary = server_script(&fixture(), build, &[]);
    let path = host.dir.path().join("dev-tenmoku-server");
    std::fs::write(&path, &binary).unwrap();
    set_mode(&path, 0o755);
    (path, binary)
}

fn no_release() -> Release {
    Arc::new(|_| panic!("a dev build has no release to fetch"))
}

#[test]
fn a_dev_server_installs_to_dev_and_its_sha256() {
    let host = Host::new();
    let (path, binary) = dev_server(&host, BUILD);
    let connector = host.connector(
        host.transport(plain_prelude()),
        ServerSource::for_build(BUILD, Some(path), no_release()),
        no_prompts(),
    );
    let states = States::default();
    let connection = connector.connect(states.recorder()).unwrap();
    assert_eq!(connection.welcome().build, BUILD);
    connection.close();

    let dir = format!("dev-{}", &sha256(&binary)[..12]);
    assert_eq!(std::fs::read(host.server(&dir)).unwrap(), binary);
    let mut expected = vec![State::Connecting, State::Probing(probe(false))];
    expected.extend(uploading(pack(&binary).unwrap().len() as u64));
    expected.extend([State::Starting, State::Connected]);
    assert_eq!(states.take(), expected);
}

#[test]
fn a_dev_server_of_another_build_ends_with_its_message() {
    let host = Host::new();
    let (path, _) = dev_server(&host, "0.3.0+9f8e7d6");
    let connector = host.connector(
        host.transport(plain_prelude()),
        ServerSource::Dev(path),
        no_prompts(),
    );
    let states = States::default();
    let failed = connector.connect(states.recorder()).err().unwrap();
    assert!(
        matches!(failed.error, ConnectError::DevServerBuild { .. }),
        "{:?}",
        failed.error
    );
    assert_eq!(
        failed.error.to_string(),
        format!(
            "remote.dev_server is build 0.3.0+9f8e7d6, this client is {BUILD}. Build it from the same checkout."
        )
    );
    let states = states.take();
    assert!(
        !states
            .iter()
            .any(|state| matches!(state, State::Reinstalling { .. })),
        "nothing is reinstalled: {states:?}"
    );
}

#[test]
fn a_dev_build_without_a_dev_server_names_the_setting() {
    let host = Host::new();
    let dirty = "0.3.0+ab12cd3-dirty";
    let source = ServerSource::for_build(dirty, None, no_release());
    assert!(matches!(source, ServerSource::NoRelease), "{source:?}");
    let connector = host.connector_as(dirty, host.transport(plain_prelude()), source, no_prompts());
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    let probe = probe(false);
    let asset = fe_remote_client::Asset::for_host(&probe.os, &probe.arch).unwrap();
    assert_eq!(
        failed.error.to_string(),
        format!(
            "This development build has no matching tenmoku-server release. Set remote.dev_server to a server built for {asset}."
        )
    );
    if asset == fe_remote_client::Asset::LinuxX86_64 {
        assert_eq!(
            failed.error.to_string(),
            "This development build has no matching tenmoku-server release. Set remote.dev_server to a server built for linux-x86_64."
        );
    }
}

#[test]
fn a_release_build_fetches_its_release() {
    let release: Release = Arc::new(|_| Ok(Archive::Bytes(Arc::from(&b""[..]))));
    let source = ServerSource::for_build("0.3.0+ab12cd3", None, release);
    assert!(matches!(source, ServerSource::Release(_)), "{source:?}");
}

#[test]
fn a_dev_build_without_a_dev_server_stops_even_with_a_server_installed() {
    let host = Host::new();
    let dirty = "0.3.0+ab12cd3-dirty";
    host.install(dirty, &server_script(&fixture(), dirty, &[]));
    let connector = host.connector_as(
        dirty,
        host.transport(plain_prelude()),
        ServerSource::for_build(dirty, None, no_release()),
        no_prompts(),
    );
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    assert!(
        matches!(failed.error, ConnectError::NoRelease { .. }),
        "{:?}",
        failed.error
    );
}

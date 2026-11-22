//! What the bootstrap may assume of a host (D § 4): POSIX sh, under 60
//! lines, and only `uname`, `mkdir`, `head -c`, `chmod`, `mv`, `tar` and
//! `sha256sum` or `shasum -a 256` (with `rm`, and the `gzip` GNU tar runs);
//! and the host's `uname` mapped to an asset, or the connect ended.

#![cfg(unix)]

mod common;

use common::{Host, States, no_prompts, release, server_archive};
use fe_remote_client::testing::{ChildTransport, plain_prelude};
use fe_remote_client::{Asset, ConnectError, bootstrap};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

/// `tool`'s path on this machine's PATH.
fn which(tool: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|dir| dir.join(tool))
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("{tool} isn't on PATH"))
}

/// A folder of links to `tools`, and nothing else, to be the whole PATH.
fn only(dir: &Path, tools: &[&str]) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir(&bin).unwrap();
    for tool in tools {
        std::os::unix::fs::symlink(which(tool), bin.join(tool)).unwrap();
    }
    bin
}

/// Runs a missing install of `dir` under `/bin/sh` with PATH `path`, after
/// three earlier builds so it prunes: stdout, and whether it succeeded.
fn install_with_path(host: &Host, path: &Path) -> (String, bool) {
    for (i, old) in ["old-1", "old-2", "old-3"].iter().enumerate() {
        let old = &host.unique(old);
        host.install(old, b"#!/bin/sh\n");
        std::fs::write(host.server(old).with_file_name("build"), format!("{old}\n")).unwrap();
        let file = std::fs::File::options()
            .write(true)
            .open(host.server(old))
            .unwrap();
        let age = std::time::Duration::from_secs(3600 * (3 - i as u64));
        file.set_modified(std::time::SystemTime::now() - age)
            .unwrap();
    }
    let archive =
        fe_remote_client::testing::archive(&[("tenmoku-server", b"#!/bin/sh\necho ran\n", 0o644)]);
    let sha: String = Sha256::digest(&archive)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(bootstrap::script("new", false).unwrap())
        .env_clear()
        .env("PATH", path)
        .envs(host.env())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    // All at once: the script reads the line and then exactly the archive.
    writeln!(stdin, "{} {sha}", archive.len()).unwrap();
    stdin.write_all(&archive).unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.is_empty(), "{stderr}");
    (stdout, out.status.success())
}

fn installs(host: &Host) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(host.cache().join("tenmoku-server"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .map(|name| host.plain(&name).to_string())
        .collect();
    names.sort();
    names
}

const TOOLS: [&str; 7] = ["uname", "mkdir", "head", "chmod", "mv", "tar", "rm"];

#[test]
fn the_script_is_posix_sh_under_60_lines() {
    let script = bootstrap::script("0.3.0+ab12cd3", false).unwrap();
    assert!(
        script.lines().count() < 60,
        "{} lines",
        script.lines().count()
    );
    // ssh hands it to the login shell in single quotes.
    assert!(!script.contains('\''));
    assert!(!script.contains("\\\\"));
    for shell in ["sh", "dash"] {
        let Some(shell) = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|dir| dir.join(shell))
            .find(|path| path.is_file())
        else {
            continue;
        };
        let status = Command::new(shell)
            .arg("-n")
            .arg("-c")
            .arg(&script)
            .status();
        assert!(status.unwrap().success());
    }
}

#[test]
fn the_script_installs_and_prunes_with_only_its_tools() {
    let host = Host::new();
    let mut tools = TOOLS.to_vec();
    tools.extend(["sha256sum", "gzip"]);
    let path = only(host.dir.path(), &tools);
    let (stdout, ok) = install_with_path(&host, &path);
    assert!(ok, "{stdout}");
    assert!(stdout.ends_with("TENMOKU-BOOT ready\nran\n"), "{stdout}");
    assert_eq!(installs(&host), ["new", "old-2", "old-3"]);
}

#[test]
fn shasum_stands_in_for_sha256sum() {
    let host = Host::new();
    let mut tools = TOOLS.to_vec();
    // shasum is a perl script.
    tools.extend(["shasum", "perl", "gzip"]);
    let path = only(host.dir.path(), &tools);
    let (stdout, ok) = install_with_path(&host, &path);
    assert!(ok, "{stdout}");
    assert!(stdout.ends_with("TENMOKU-BOOT ready\nran\n"), "{stdout}");
}

#[test]
fn uname_values_map_to_the_asset_names() {
    let cases = [
        ("Linux", "x86_64", Some("linux-x86_64")),
        ("Linux", "amd64", Some("linux-x86_64")),
        ("Linux", "aarch64", Some("linux-aarch64")),
        ("Linux", "arm64", Some("linux-aarch64")),
        ("Darwin", "arm64", Some("macos-arm64")),
        ("Darwin", "aarch64", Some("macos-arm64")),
        ("Darwin", "x86_64", None),
        ("Linux", "riscv64", None),
        ("FreeBSD", "amd64", None),
        ("MINGW64_NT-10.0", "x86_64", None),
    ];
    for (os, arch, asset) in cases {
        assert_eq!(
            Asset::for_host(os, arch).map(Asset::name),
            asset,
            "{os} {arch}"
        );
    }
}

#[test]
fn an_unsupported_host_ends_the_connect() {
    let host = Host::new();
    let bin = host.dir.path().join("fake-uname");
    std::fs::create_dir(&bin).unwrap();
    let uname = bin.join("uname");
    std::fs::write(
        &uname,
        "#!/bin/sh\ncase $1 in -s) echo FreeBSD;; -m) echo amd64;; esac\n",
    )
    .unwrap();
    common::set_mode(&uname, 0o755);
    let mut path = OsString::from(&bin);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap());
    let mut env = host.env();
    env.push(("PATH".into(), path));
    let transport = Arc::new(ChildTransport {
        prelude: plain_prelude(),
        env,
    });
    let archive = server_archive(fe_remote_proto::BUILD, &[], 0);
    let connector = host.connector(transport, release(archive), no_prompts());
    let failed = connector
        .connect(States::default().recorder())
        .err()
        .unwrap();
    assert!(
        matches!(failed.error, ConnectError::Unsupported { .. }),
        "{:?}",
        failed.error
    );
    assert_eq!(
        failed.error.to_string(),
        "tenmoku-server isn't built for FreeBSD amd64."
    );
    assert!(
        !host
            .cache()
            .join("tenmoku-server")
            .join(fe_remote_proto::BUILD)
            .join("tenmoku-server")
            .exists()
    );
}

//! The bootstrap script (D § 4) run under `sh` here, with a temporary HOME
//! and XDG_CACHE_HOME, driven line by line as the client drives it.

#![cfg(unix)]

mod common;

use common::{Host, probe, set_mode};
use fe_remote_client::bootstrap;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::time::{Duration, SystemTime};

/// A server that says it ran, and with what. As `--version` it reports its
/// folder's name for its build, as a release install's server does.
const SERVER: &[u8] = b"#!/bin/sh\n\
    case $1 in --version) d=${0%/*}; echo \"tenmoku-server ${d##*/}\" ;;\n\
    *) echo \"server $*\" ;; esac\n";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn server_archive() -> Vec<u8> {
    archive_of(SERVER)
}

fn archive_of(server: &[u8]) -> Vec<u8> {
    fe_remote_client::testing::archive(&[("tenmoku-server", server, 0o644)])
}

/// The script running under `sh`.
struct Boot {
    child: Child,
    out: BufReader<ChildStdout>,
    stdin: Option<ChildStdin>,
}

impl Boot {
    fn start(host: &Host, dir: &str, reinstall: bool, env: &[(&str, OsString)]) -> Boot {
        let script = bootstrap::script(dir, reinstall).unwrap();
        let mut command = Command::new("sh");
        command.arg("-c").arg(script).env_clear();
        command.env("PATH", std::env::var_os("PATH").unwrap());
        command.envs(host.env());
        command.envs(env.iter().map(|(k, v)| (k, v)));
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        Boot {
            out: BufReader::new(child.stdout.take().unwrap()),
            stdin: child.stdin.take(),
            child,
        }
    }

    fn line(&mut self) -> String {
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        line.trim_end().to_owned()
    }

    fn upload(&mut self, bytes: &[u8], sha256: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{} {sha256}", bytes.len()).unwrap();
        stdin.write_all(bytes).unwrap();
        stdin.flush().unwrap();
    }

    fn finish(mut self) -> ExitStatus {
        drop(self.stdin.take());
        self.child.wait().unwrap()
    }
}

fn probe_line(present: bool) -> String {
    let probe = probe(present);
    let state = if present { "present" } else { "missing" };
    format!("TENMOKU-BOOT 1 {} {} {state}", probe.os, probe.arch)
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Installs `dirs` as earlier builds, the first the oldest, an hour apart,
/// each with its build file.
fn earlier_builds(host: &Host, dirs: &[&str]) {
    let ages: Vec<u64> = (1..=dirs.len() as u64).rev().collect();
    earlier_builds_aged(host, dirs, &ages);
}

/// Installs `dirs` as earlier builds, each `hours` old.
fn earlier_builds_aged(host: &Host, dirs: &[&str], hours: &[u64]) {
    let now = SystemTime::now();
    for (dir, &age) in dirs.iter().zip(hours) {
        host.install(dir, SERVER);
        let server = host.server(dir);
        std::fs::write(server.with_file_name("build"), format!("{dir}\n")).unwrap();
        let file = std::fs::File::options().write(true).open(server).unwrap();
        file.set_modified(now - Duration::from_secs(3600 * age))
            .unwrap();
    }
}

/// Holds `build`'s daemon lock under HOME, as its daemon would, with `pid`
/// written in it.
fn hold_lock(host: &Host, build: &str, pid: Option<u32>) -> std::fs::File {
    let paths = tenmoku_server::Paths::new(build, None, Some(&host.home())).unwrap();
    paths.secure().unwrap();
    let mut lock = paths.take_lock().unwrap().unwrap();
    if let Some(pid) = pid {
        writeln!(lock, "{pid}").unwrap();
    }
    lock
}

/// Installs `new` over the wire, as the client does.
fn install(host: &Host, new: &str, env: &[(&str, OsString)]) {
    let archive = server_archive();
    let mut boot = Boot::start(host, new, false, env);
    assert_eq!(boot.line(), probe_line(false));
    boot.upload(&archive, &sha256(&archive));
    assert_eq!(boot.line(), "TENMOKU-BOOT ready");
    assert_eq!(boot.line(), "server --stdio");
    assert!(boot.finish().success());
}

fn installs(host: &Host) -> Vec<String> {
    entries(&host.cache().join("tenmoku-server"))
}

#[test]
fn a_missing_server_is_uploaded_checked_unpacked_and_execd() {
    let host = Host::new();
    let archive = server_archive();
    let mut boot = Boot::start(&host, "1.0.0+abc", false, &[]);
    assert_eq!(boot.line(), probe_line(false));
    boot.upload(&archive, &sha256(&archive));
    assert_eq!(boot.line(), "TENMOKU-BOOT ready");
    assert_eq!(boot.line(), "server --stdio", "it execs the server");
    assert!(boot.finish().success());

    let server = host.server("1.0.0+abc");
    assert_eq!(std::fs::read(&server).unwrap(), SERVER);
    let mode = std::fs::metadata(&server).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
    assert_eq!(
        entries(server.parent().unwrap()),
        ["build", "tenmoku-server"],
        "no upload left behind"
    );
    let build = std::fs::read_to_string(server.with_file_name("build")).unwrap();
    assert_eq!(build, "1.0.0+abc\n", "the build the server reports");
    assert!(!host.home().join(".cache").exists(), "XDG_CACHE_HOME wins");
}

#[test]
fn without_xdg_cache_home_it_installs_under_home() {
    let host = Host::new();
    let archive = server_archive();
    let mut boot = Boot::start(&host, "1.0.0+abc", false, &[("XDG_CACHE_HOME", "".into())]);
    assert_eq!(boot.line(), probe_line(false));
    boot.upload(&archive, &sha256(&archive));
    assert_eq!(boot.line(), "TENMOKU-BOOT ready");
    boot.finish();
    let server = host
        .home()
        .join(".cache/tenmoku-server/1.0.0+abc/tenmoku-server");
    assert_eq!(std::fs::read(server).unwrap(), SERVER);
}

#[test]
fn a_present_server_is_execd_with_no_upload() {
    let host = Host::new();
    host.install("1.0.0+abc", SERVER);
    let mut boot = Boot::start(&host, "1.0.0+abc", false, &[]);
    assert_eq!(boot.line(), probe_line(true));
    // Nothing is sent: stdin stays open and empty.
    assert_eq!(boot.line(), "TENMOKU-BOOT ready");
    assert_eq!(boot.line(), "server --stdio");
    assert!(boot.finish().success());
}

#[test]
fn a_reinstall_replaces_a_present_server() {
    let host = Host::new();
    host.install("1.0.0+abc", b"#!/bin/sh\necho stale\n");
    let archive = server_archive();
    let mut boot = Boot::start(&host, "1.0.0+abc", true, &[]);
    assert_eq!(boot.line(), probe_line(true));
    boot.upload(&archive, &sha256(&archive));
    assert_eq!(boot.line(), "TENMOKU-BOOT ready");
    assert_eq!(boot.line(), "server --stdio");
    boot.finish();
    assert_eq!(std::fs::read(host.server("1.0.0+abc")).unwrap(), SERVER);
}

#[test]
fn a_wrong_sha256_installs_nothing() {
    let host = Host::new();
    let archive = server_archive();
    let wrong = sha256(b"something else");
    let mut boot = Boot::start(&host, "1.0.0+abc", false, &[]);
    assert_eq!(boot.line(), probe_line(false));
    boot.upload(&archive, &wrong);
    assert_eq!(
        boot.line(),
        format!(
            "TENMOKU-BOOT error the upload has SHA-256 {}, not {wrong}",
            sha256(&archive)
        )
    );
    assert_eq!(boot.line(), "", "and nothing after");
    assert!(!boot.finish().success());
    let dir = host.server("1.0.0+abc");
    assert_eq!(entries(dir.parent().unwrap()), Vec::<String>::new());
}

#[test]
fn an_archive_without_a_server_installs_nothing() {
    let host = Host::new();
    let archive = fe_remote_client::testing::archive(&[("other", SERVER, 0o755)]);
    let mut boot = Boot::start(&host, "1.0.0+abc", false, &[]);
    assert_eq!(boot.line(), probe_line(false));
    boot.upload(&archive, &sha256(&archive));
    let line = boot.line();
    assert!(
        line.starts_with("TENMOKU-BOOT error can not unpack the upload"),
        "{line}"
    );
    assert!(!boot.finish().success());
    let dir = host.server("1.0.0+abc");
    assert_eq!(entries(dir.parent().unwrap()), Vec::<String>::new());
}

#[test]
fn pruning_keeps_the_newest_three_builds() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-3", "old-4"]);
}

#[test]
fn pruning_skips_a_build_whose_daemon_lock_is_held() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    // As the daemon holds it: HOME's fallback when there's no runtime dir.
    let lock = hold_lock(&host, "old-1", Some(std::process::id()));
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-1", "old-3", "old-4"]);
    drop(lock);
}

#[test]
fn pruning_skips_a_build_whose_daemon_lock_is_held_in_the_runtime_dir() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    let runtime = host.dir.path().join("run");
    std::fs::create_dir(&runtime).unwrap();
    set_mode(&runtime, 0o700);
    let paths = tenmoku_server::Paths::new("old-2", Some(&runtime), None).unwrap();
    paths.secure().unwrap();
    let mut lock = paths.take_lock().unwrap().unwrap();
    writeln!(lock, "{}", std::process::id()).unwrap();
    install(&host, "new", &[("XDG_RUNTIME_DIR", runtime.into())]);
    assert_eq!(installs(&host), ["new", "old-2", "old-3", "old-4"]);
    drop(lock);
}

#[test]
fn pruning_keeps_a_build_whose_lock_file_is_left() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    // A lock left by a daemon that has exited: nothing sh can see shows it
    // free, so it may be a daemon's on its way up.
    let pid = dead_pid();
    let paths = tenmoku_server::Paths::new("old-1", None, Some(&host.home())).unwrap();
    paths.secure().unwrap();
    std::fs::write(&paths.lock, format!("{pid}\n")).unwrap();
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-1", "old-3", "old-4"]);
}

#[test]
fn pruning_removes_a_build_once_its_runtime_dir_is_cleared() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    let runtime = host.dir.path().join("run");
    std::fs::create_dir(&runtime).unwrap();
    set_mode(&runtime, 0o700);
    let paths = tenmoku_server::Paths::new("old-1", Some(&runtime), None).unwrap();
    paths.secure().unwrap();
    std::fs::write(&paths.lock, "1\n").unwrap();
    // The user's last session ended: the runtime dir goes, locks and all.
    std::fs::remove_dir_all(&runtime).unwrap();
    std::fs::create_dir(&runtime).unwrap();
    install(&host, "new", &[("XDG_RUNTIME_DIR", runtime.into())]);
    assert_eq!(installs(&host), ["new", "old-3", "old-4"]);
}

/// The pid of a process that has exited.
fn dead_pid() -> u32 {
    let mut ended = Command::new("true").spawn().unwrap();
    let pid = ended.id();
    ended.wait().unwrap();
    pid
}

#[test]
fn pruning_keeps_a_build_whose_held_lock_names_a_dead_pid() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    // Taken by a daemon on its way up, the last daemon's pid not yet
    // rewritten.
    let lock = hold_lock(&host, "old-1", Some(dead_pid()));
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-1", "old-3", "old-4"]);
    drop(lock);
}

#[test]
fn a_dev_install_is_pruned_by_the_build_its_daemon_has() {
    let host = Host::new();
    earlier_builds(&host, &["dev-0123456789ab", "old-2", "old-3", "old-4"]);
    let dev = host.cache().join("tenmoku-server/dev-0123456789ab/build");
    std::fs::write(dev, "1.0.0+abc-dirty\n").unwrap();
    let lock = hold_lock(&host, "1.0.0+abc-dirty", Some(std::process::id()));
    install(&host, "new", &[]);
    assert_eq!(
        installs(&host),
        ["dev-0123456789ab", "new", "old-3", "old-4"]
    );
    drop(lock);
}

#[test]
fn the_install_records_the_build_its_server_reports() {
    let host = Host::new();
    let server = b"#!/bin/sh\necho \"tenmoku-server 0.3.0+9f8e7d6\"\n";
    let archive = archive_of(server);
    let mut boot = Boot::start(&host, "dev-0123456789ab", false, &[]);
    assert_eq!(boot.line(), probe_line(false));
    boot.upload(&archive, &sha256(&archive));
    assert_eq!(boot.line(), "TENMOKU-BOOT ready");
    boot.finish();
    let build = host.server("dev-0123456789ab").with_file_name("build");
    assert_eq!(std::fs::read_to_string(build).unwrap(), "0.3.0+9f8e7d6\n");
}

#[test]
fn pruning_keeps_a_build_whose_held_lock_has_no_pid_yet() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    // Taken, and not yet written: the daemon is starting.
    let lock = hold_lock(&host, "old-1", None);
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-1", "old-3", "old-4"]);
    drop(lock);
}

#[test]
fn pruning_keeps_an_install_whose_build_is_unknown() {
    let host = Host::new();
    earlier_builds(&host, &["old-1", "old-2", "old-3", "old-4"]);
    std::fs::remove_file(host.server("old-1").with_file_name("build")).unwrap();
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-1", "old-3", "old-4"]);
}

#[test]
fn pruning_breaks_ties_in_modification_time_by_name() {
    let host = Host::new();
    let olds = ["old-1", "old-2", "old-3", "old-4", "old-5"];
    earlier_builds_aged(&host, &olds, &[1; 5]);
    install(&host, "new", &[]);
    assert_eq!(installs(&host), ["new", "old-4", "old-5"]);
}

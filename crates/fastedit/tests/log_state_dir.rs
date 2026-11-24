#![cfg(unix)]

use std::process::{Command, Stdio};

#[test]
fn a_state_dir_launch_logs_inside_it() {
    let home = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let xdg = home.path().join("xdg");
    let status = Command::new(env!("CARGO_BIN_EXE_tenmoku"))
        .arg("--version")
        .env("TENMOKU_NO_IPC", "1")
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", &xdg)
        .env("TENMOKU_STATE_DIR", state.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap()
        .status;
    assert!(status.success());
    let log = state.path().join("tenmoku.log");
    assert!(log.is_file(), "no log in the state dir");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.lines().next().unwrap().contains("launch: version"));
    assert!(
        !home
            .path()
            .join("Library/Logs/Tenmoku/tenmoku.log")
            .exists()
    );
    assert!(!xdg.join("tenmoku/tenmoku.log").exists());
}

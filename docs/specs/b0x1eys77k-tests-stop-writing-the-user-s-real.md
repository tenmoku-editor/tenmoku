---
task_uid: "01m84vnkr0p20j6tb0x1eys77k"
task: "T415"
rev: 1
type: "bugfix"
complexity: "xs"
criteria:
  - id: "C1"
    text: "`a_state_dir_launch_logs_inside_it` passes (fastedit integration test log_state_dir.rs)"
    required: true
    manual: false
  - id: "C2"
    text: "`log_file::tests::log_path_honours_the_state_dir` and `log_file::tests::install_follows_the_real_stderr` pass"
    required: true
    manual: false
---

# Tests stop writing the user's real tenmoku.log: the log honours TENMOKU_STATE_DIR (Mac T2111)

# Tests stop writing the user's real tenmoku.log

`~/Library/Logs/Tenmoku/tenmoku.log` on the lead's Mac is mostly test noise: about 3,000 `launch: version … kind foreground displays 0` lines and 1,000 `[ERROR fastedit] ipc: the editor went before the files closed: unexpected end of file`, in runs of three or four launches then one IPC error. That hides real errors from the user (see the language server health cards).

## Cause (confirmed, main 7d75582a0)

- `crates/fastedit/src/log_file.rs` `install` points fd 2 at the log when fd 2 is `/dev/null`, and `log_path` reads only `HOME` (macOS: `$HOME/Library/Logs/Tenmoku/tenmoku.log`) or `XDG_STATE_HOME` / `HOME` (Linux: `<state>/tenmoku/tenmoku.log`). It ignores `TENMOKU_STATE_DIR`.
- `crates/fastedit/tests/wait_close.rs` `client_process` runs the real binary (`--wait a.txt`) with `.stderr(Stdio::null())` and the test's own `HOME`. Its three launches (`client_disconnect_forgets_the_wait`, and `told` + `stranded` in `the_client_exits_0_when_told_and_1_when_the_server_crashes`) write launch lines to the real log, and `stranded` writes the IPC error (`crates/fastedit/src/ipc.rs`, the server is gone before the files closed). The other tests that run the binary pipe stderr (`instance_handoff.rs`, `location_handoff.rs`, `l10n.rs`, `version_flag.rs`), so they never redirect.
- Every nextest run already sets its own `TENMOKU_STATE_DIR` (`scripts/nextest-isolate-state.sh`, `.config/nextest.toml`, T1664), so the log is the one piece of state that escapes it.

## Fix

`log_path` takes the log from `$TENMOKU_STATE_DIR/tenmoku.log` when that is set and non-empty (as `fe_config::paths::state_dir_from` treats it), on every platform, before the `HOME` / `XDG_STATE_HOME` rules. No test needs to change: they inherit nextest's state dir. Update the module docs to say so.

## Tests

- Regression (base API only): `crates/fastedit/tests/log_state_dir.rs` (`#![cfg(unix)]`), `a_state_dir_launch_logs_inside_it`: runs `env!("CARGO_BIN_EXE_tenmoku") --version` with `TENMOKU_NO_IPC=1`, `HOME` and `XDG_STATE_HOME` in one temp dir, `TENMOKU_STATE_DIR` in another, stdout piped and `.stderr(Stdio::null())`, waits for it to exit 0, then `assert!(state.join("tenmoku.log").is_file())` and that its first line contains `launch: version`, and that neither `<home>/Library/Logs/Tenmoku/tenmoku.log` nor `<xdg>/tenmoku/tenmoku.log` exists. Base: the log lands under `HOME` (macOS) or `XDG_STATE_HOME` (Linux), so the first `assert!` fails.
- Unit, in `crates/fastedit/src/log_file/tests.rs`, `log_path_honours_the_state_dir`: `log_path` with `TENMOKU_STATE_DIR` set, set but empty (falls back), and unset.

Do not widen --scope; if a file outside it is needed, stop and say so.

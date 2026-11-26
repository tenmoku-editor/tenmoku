---
task_uid: "01m852gz6ew5p0yfn2ke8pgtaw"
task: "T417"
rev: 1
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "`tests::lsp_message_tone::info_message_shows_in_the_default_tone` passes (fe-workbench): Info -\u003e StatusTone::Default, Warning -\u003e Warning, Error -\u003e Error"
    required: true
    manual: false
  - id: "C2"
    text: "`tests::stderr_errors::error_lines_reach_the_app_log_at_warn`, `tests::stderr_errors::a_line_split_across_reads_is_forwarded_once`, `tests::stderr_errors::forwarding_stops_at_the_cap_with_one_notice` and `tests::stderr_errors::error_log_messages_are_warned` pass (fe-lsp)"
    required: true
    manual: false
  - id: "C3"
    text: "`tests::lsp::restart_and_show_log` and `tests::lsp::messages_are_per_server` still pass (fe-workbench)"
    required: true
    manual: false
---

# Language server errors on stderr reach tenmoku.log, and Info messages stop looking like errors (Mac T2109)

# Language server errors on stderr reach tenmoku.log, and Info messages stop looking like errors

User report: rust-analyzer was failing with nothing visible; the app log (`~/Library/Logs/Tenmoku/tenmoku.log`, INFO level, `crates/fastedit/src/log_file.rs`) had no trace of its errors either.

## Bugs (main 7d75582a0)

1. `crates/fe-lsp/src/server.rs` `drain_stderr` pushes each stderr chunk into the 64 KiB in-memory ring and logs it at `log::debug!`, which the app's INFO logger drops. rust-analyzer's `ERROR` lines and panics are only in the ring, gone when the app quits. `window/logMessage` (`crates/fe-lsp/src/router.rs` `Router::notified`) also goes only to the ring, at every `MessageType`.
2. `crates/fe-workbench/src/lsp.rs`: `messages` keeps only the `window/showMessage` text, and `status_text` shows any message with `StatusTone::Error`, so an Info message ("Loaded 3 crates") draws as an error.

## Fix

- `drain_stderr`: split what it reads into lines (a line may span reads; keep the partial tail, bounded at the ring's size) and forward each line that looks like an error (contains ` ERROR `, starts with `ERROR`, or contains `panicked`) to `log::warn!("{name} stderr: {line}")`. The ring and the debug line stay as they are. Cap the forwarded lines per session (e.g. 50), then log one warn line saying further error output is only in the server's log (`Show Language Server Log`).
- `router.rs`: a `window/logMessage` of `MessageType::ERROR` also goes to `log::warn!` (same per-session cap, shared or its own), still into the ring.
- `crates/fe-workbench/src/lsp.rs`: keep the message's level with it (`messages: HashMap<ServerId, (level, String)>`; fe-workbench has no lsp_types of its own, so read the level as the existing toast code does) and pass it to `status_text`: Error -> `StatusTone::Error`, Warning -> `StatusTone::Warning`, Info and Log -> `StatusTone::Default`. Update `force_lsp_status` and `run_server_op` callers. Toast behaviour is unchanged.
- `crates/fe-lsp/src/tests/mod.rs`: the test logger (`Capture`) also records each record's `log::Level`, with a helper returning a thread's records at a level; `logs_from` keeps its signature.

## Regression tests (base API only)

- `crates/fe-workbench/src/tests/lsp_message_tone.rs`, declared from `crates/fe-workbench/src/tests/mod.rs`, on the `crate::tests::lsp::Lsp` harness: `info_message_shows_in_the_default_tone` opens `src/main.rs`, waits for the fake to initialize, sends `ShowMessage` `MessageType::INFO` "Loaded 3 crates", `s.wait_item(Some("Loaded 3 crates"))`, then `assert_eq!` the tone of the item with id `crate::lsp::ITEM` in `w.status_bar().read(cx).items(cx)` against `StatusTone::Default`; then WARNING -> `StatusTone::Warning`, ERROR -> `StatusTone::Error`. Base: the Info message has `StatusTone::Error`, so the first `assert_eq!` fails.
- `crates/fe-lsp/src/tests/stderr_errors.rs`, declared from `crates/fe-lsp/src/tests/mod.rs`: `error_lines_reach_the_app_log_at_warn` captures the `fe-lsp <name>` thread, has the fake write `"2026-11-23T10:00:00Z ERROR failed to run build scripts\n"` and an ordinary `"INFO indexing\n"` line to stderr, waits until `manager.log(name, root)` contains both (the ring has them), then `assert_eq!`s the warn-level records to exactly one containing `failed to run build scripts`. Base: no warn record, so the `assert_eq!` fails. Do not wait on the warn record itself: a timeout panic is not an assertion at base.
- Also (not regression), same file: `a_line_split_across_reads_is_forwarded_once`, `forwarding_stops_at_the_cap_with_one_notice` (past the cap, one notice and no more), `error_log_messages_are_warned` (a `logMessage` ERROR is warned, an INFO one is not).

Do not widen --scope; if a file outside it is needed, stop and say so.

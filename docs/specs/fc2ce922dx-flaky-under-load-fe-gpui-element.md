---
task_uid: "01m82pdcxsns9gnefc2ce922dx"
task: "T412"
rev: 1
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "one_limit_per_frame no longer depends on how much search the host completes in a fixed wall-clock limit, and still proves one limit per frame with the matches painted on the first row"
    required: true
    manual: false
  - id: "C2"
    text: "cargo nextest run -p fe-gpui highlight_limit_tests passes 20 times in a row while another full-workspace test run loads the host"
    required: true
    manual: false
  - id: "C3"
    text: "Only crates/fe-gpui/src/element/highlight_limit_tests.rs changes"
    required: true
    manual: false
---

# Flaky under load: fe-gpui element::highlight_limit_tests::one_limit_per_frame (100 ms wall-clock limit paints no match; assertion !painted.is_empty()) (Mac T2101)

Main's post-merge rerun of b1af6984a (run 01m82n5br73kz2csrhcy12wcrh, full workspace nextest, 12068 tests, host load ~15) failed:

    crates/fe-gpui/src/element/highlight_limit_tests.rs:72: assertion failed: !painted.is_empty()

one_limit_per_frame sets TEST_LIMIT to 100 ms on 20 rows of 200,000 'a's with the pattern `a.*z\|a`, and expects the frame to paint at least one match before the limit runs out. Under load the backtracking `a.*z` branch spends the whole 100 ms of wall clock before any match is found, so nothing is painted.

T621 (92a75915f) fixed the sibling test ran_out_latches_until_a_change in the same file by making its limits load-independent (Duration::ZERO to force running out, an hour to never run out). Do the same for one_limit_per_frame: keep what it proves (one limit per frame, the frame stops searching when it runs out, the painted matches are all on the first row and placed at its top, the frame returns promptly) without depending on how much the host gets done in 100 ms. For example, size the work so one row's search fits well inside the limit on a loaded host while the whole buffer can't, or check the bound by counting rows searched (FOUND or a row counter) rather than by wall-clock.

Test-only: only crates/fe-gpui/src/element/highlight_limit_tests.rs changes.

---
task_uid: "01m80p3pjhrs6b7bkmmq1zmqsk"
task: "T435"
rev: 1
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "fe-dap tests::session_resume::a_step_without_continued_runs_on_its_response no longer reads a transient state: it checks Resuming/Running deterministically (gated fake or recorded states), and still proves a step without continued runs on its response"
    required: true
    manual: false
  - id: "C2"
    text: "50 consecutive runs of the fe-dap session_resume tests under load all pass, counts quoted in the implementation notes"
    required: true
    manual: false
  - id: "C3"
    text: "cargo clippy -p fe-dap --all-targets and cargo nextest run -p fe-dap pass"
    required: true
    manual: false
---

# fe-dap session_resume flake: a_step_without_continued_runs_on_its_response reads transient controls after resume (fails on GitHub Linux) (Mac T2092)

`fe-dap`'s `tests::session_resume::a_step_without_continued_runs_on_its_response` fails on GitHub's Linux runner: it panicked at crates/fe-dap/src/tests/session_resume.rs:80:5 in both Linux passes of main CI run on d7c4f0103 (02:34:00Z and 02:50:17Z, 0.013 s each), with `controls.step` Enabled where the test expects Disabled(Resuming).

Cause (triage, no code change since 11-16): the test reads `controls()` after `resume()` returns, but the fake adapter answers `next` and sends `stopped` at once, so on a fast runner the session is already Stopped at the new epoch, controls Enabled again, before the test thread looks. The assertion reads a transient state. The D2b stop/resume tests (e91dc6f2f, T654) are the origin.

Fix the test, not production code: make the fake's step response and `stopped` event wait on a gate the test releases after it has checked the controls and the stale variables(), or assert the Resuming/Running states from the session's recorded state list instead of live controls(). Look for the same transient-read pattern in the other session_resume tests and fix any instance of it the same way. Do not loosen what the test proves: a step without `continued` must still run on its response.

Test-only fix, so no regression test: prove it with repeated runs under load (e.g. 50 consecutive runs of the fe-dap session_resume tests with the machine loaded, all passing), and quote the counts.

## Implementation notes

- `a_step_without_continued_runs_on_its_response` now gives the fake a `next` fault, `ReplyAfter(Trigger::Manual)`. The step's response and the new `stopped` event are held until the test calls `fake.release()`. Before the release the test checks the session state and `controls.step` are Resuming. After it, the Resuming → Running → Stopped state list and the no-`continued` checks are unchanged.
- The other session_resume tests read no transient state: their live reads follow waits or hold by design. `fake.rs` is unchanged.
- C2: 50 consecutive runs of `cargo nextest run -p fe-dap session_resume` on a 12-core host with 32 busy loops running. The 1-minute load average was 33.79 to 36.86 across the runs. Result: 50 of 50 passed (11 of 11 tests each). The per-run log with the load average is at `scratchpad/runs.log`.
- C3: `cargo clippy -p fe-dap --all-targets` is clean, and `cargo nextest run -p fe-dap` passed 209 of 209.

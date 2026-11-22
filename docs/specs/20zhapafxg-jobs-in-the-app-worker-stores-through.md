---
task_uid: "01m71szcc6453njr20zhapafxg"
task: "T1704"
rev: 6
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "fe-workbench runs worker stores through the component cache; plugin jobs appear in T1570's Runs with `RunOwner::Plugin`, `RunSummary::Text`, a timed-out reason and `RunStatus::Queued` (waiting reason, `queued_at`, cancel before start)."
    required: true
    manual: false
  - id: "C2"
    text: "New tests in crates/fe-workbench/src/plugin_jobs_tests.rs: `queued_job_shows_waiting_and_cancels_without_starting`, `queued_to_running_sets_started`, `progress_text_is_shown`, `timed_out_job_shows_reason`, `jobs_cancelled_on_unload_reload_disable_and_quit`."
    required: true
    manual: false
  - id: "C3"
    text: "Progress text from a plugin is length-capped by the host before it reaches Runs."
    required: true
    manual: false
---

# Jobs in the app: worker stores through the cache, plugin jobs in Runs (B6, 3.5)

# Jobs in the app: worker stores through the cache, plugin jobs in Runs (B6, 3.5)

Draft id B6 of the plugins stage 3 plan (docs/decisions/dqsjfernkt-plugins-stage-3.md, section 6). The design is approved by the user (2026-11-09); questions Q1 to Q7 follow the design's proposals.

Section 3.5. Adds the four items section 3.5 says T1570's model needs; if T1570 has not landed the worker must wait (depends-on).

Read the design sections named above before starting. Do not widen --scope; if a file outside it is needed, stop and say so.

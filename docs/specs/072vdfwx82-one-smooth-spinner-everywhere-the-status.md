---
task_uid: "01m8ceh8t0d5jd69072vdfwx82"
task: "T2123"
rev: 2
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "fe-workbench test the_operations_spinner_turns_with_time_not_ticks: with an operation running, the status-bar spinner angle after 50 ms of elapsed time differs from its angle at 0 ms and a frame is asked for while it draws (fails at the base, where the angle moves only on the 100 ms tick)"
    required: true
    manual: false
  - id: "C2"
    text: "Every Icon::LoaderCircle site listed in the spec draws through one kiln-ui Spinner; the Runs panel spin_angle is moved there, not duplicated"
    required: true
    manual: false
  - id: "C3"
    text: "With reduce_motion on, every spinner is still and asks for no frames (a test per kind of site or one over the shared element)"
    required: true
    manual: false
  - id: "C4"
    text: "The decision file records the CPU of an idle window showing one spinner before and after, and the frame cap if one was needed"
    required: true
    manual: false
---

# One smooth spinner everywhere: the status-bar operations spinner steps 10 times a second; turn every LoaderCircle at display rate from one shared kiln-ui Spinner

# One smooth spinner everywhere

USER report (11-26): "Can we make the progress circle smoother as well? Looks a bit janky."

## Cause

- The status bar's operations item (`crates/fe-workbench/src/operations_status.rs`: language servers starting,
  indexing, installs) turns its spinner in **10 steps a second**: `TICK` = 100 ms, `TURN` = 10, so it jumps 36°
  at a time. That is the janky one.
- The Runs panel already does it right (`runs_panel.rs` `spin_angle` + `request_animation_frame`): the angle
  comes from elapsed time and a frame is asked for each draw, so it turns at the display's rate.
- Other sites show `Icon::LoaderCircle` (run widget, debug run widget, terminal tabs, pre-commit checks, chat
  tool rows and plan rows). Check each: any that draws it without turning it is a still broken circle, which
  reads as stuck.

## Fix

- One `Spinner` element in kiln-ui: angle from elapsed time against one process-wide epoch (so every spinner on
  screen turns together), one turn per second, a frame asked for only while it is drawn, still when Reduce
  Motion is on. Move the Runs panel's `spin_angle` there rather than writing a second one.
- Use it at every `LoaderCircle` site above. The operations item keeps its 100 ms timer for its label and bar,
  and its spinner turns on frames.
- Cost: measure the CPU of an otherwise idle window showing one spinner, before (Runs panel) and after (status
  bar), with `top`/`powermetrics` over 30 s on the Mac, and record it in the decision file. If it is more than
  a few percent of one core, cap the spinner's frames at 60 a second and say so.

Part of T2119 (motion and feel); independent of its other cards.

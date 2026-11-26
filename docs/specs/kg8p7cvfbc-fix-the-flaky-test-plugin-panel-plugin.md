---
task_uid: "01m8chx882c49b2fkg8p7cvfbc"
task: "T752"
rev: 2
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "plugin_panel::plugin_panel_realtime_tests::a_running_game_gets_keys_and_is_unloaded_when_a_tick_overruns passes 20 consecutive runs under load alongside the integration suite, with the evidence in the decision file"
    required: true
    manual: false
  - id: "C2"
    text: "The fix changes only test code, or the task is retyped to a bugfix with a regression test"
    required: true
    manual: false
---

# Fix the flaky test `plugin_panel::plugin_panel_realtime_tests::a_running_game_gets_keys_and_is_unloaded_when_a_tick_overruns`, which passed only on reruns

fe-workbench.plugin_panel::plugin_panel_realtime_tests::a_running_game_gets_keys_and_is_unloaded_when_a_tick_overruns failed in a gate and passed only on its reruns.

- Gate run: 01m8chgy0jx3rsn29h25p8w3c0
- Candidate: 01m8cfpheehf4vwmwk2aykqs0f
- Task: T747
- Rerun results:
  - rerun 1: passed
  - rerun 2: passed
  - rerun 3: passed

Fix the test, not the code under it. Prove the fix with repeated runs under load: `go test -count=20` on the test alongside the integration suite. Record the evidence in the task's decision file (docs/decisions/<task_key>-<slug>.md).

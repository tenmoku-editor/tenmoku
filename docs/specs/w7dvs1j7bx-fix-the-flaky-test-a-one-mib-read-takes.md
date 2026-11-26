---
task_uid: "01m8chpr0e6f52j1w7dvs1j7bx"
task: "T751"
rev: 2
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "a_one_mib_read_takes_one_round_trip passes 20 consecutive runs under load alongside the integration suite, with the evidence in the decision file"
    required: true
    manual: false
  - id: "C2"
    text: "The fix changes only test code, or the task is retyped to a bugfix with a regression test"
    required: true
    manual: false
---

# Fix the flaky test `a_one_mib_read_takes_one_round_trip`, which passed only on reruns

fe-remote-client::remote_fs.a_one_mib_read_takes_one_round_trip failed in a gate and passed only on its reruns.

- Gate run: 01m8ch2ffevradt34gpkb9x0e0
- Candidate: 01m8cfj7gg540p07sd0v7y38vh
- Task: T748
- Rerun results:
  - rerun 1: passed
  - rerun 2: passed
  - rerun 3: passed

Fix the test, not the code under it. Prove the fix with repeated runs under load: `go test -count=20` on the test alongside the integration suite. Record the evidence in the task's decision file (docs/decisions/<task_key>-<slug>.md).

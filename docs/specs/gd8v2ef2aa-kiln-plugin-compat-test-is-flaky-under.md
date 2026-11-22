---
task_uid: "01m806ergdcx2pddgd8v2ef2aa"
task: "T434"
rev: 1
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: "In kiln/crates/kiln-plugin/tests/compat.rs, load() passes Limits { call_deadline: Duration::from_secs(10), ..Limits::default() } instead of Limits::default(), so compat_1_0_plugin_runs_on_1_1_host and the other compat tests that call answers() no longer fail with 'a call ran past its 100ms deadline' under load; no change under kiln/crates/kiln-plugin/src"
    required: true
    manual: false
  - id: "C2"
    text: "cargo test -p kiln-plugin --test compat passes 20 times in a row while the machine is loaded (e.g. several parallel cargo builds or yes\u003e/dev/null loops, load above 15)"
    required: true
    manual: false
  - id: "C3"
    text: "Tests only: the production CALL_DEADLINE (100 ms) and its own deadline tests in kiln-plugin/src/tests are unchanged"
    required: true
    manual: false
---

# kiln-plugin compat test is flaky under load: compat_1_0_plugin_runs_on_1_1_host hits the 100 ms call deadline; give the test's plugin a loose deadline (Mac T2090)

{"mode": "inline", "body": "", "ref": "", "ref_blob": ""}

## Implementation notes

C2 evidence: 20 consecutive `cargo test -p kiln-plugin --test compat` runs under 48 `yes` busy loops on 12 cores. All 20 passed 8/8. 1-min load average was 18.1 before run 1 and rose to 37.6 by run 20, above 15 throughout. An earlier series peaked at 13.2 and was discarded. Per-run results and loads:

```
run 1: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
  load 20.69 9.32 6.37
run 2: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
  load 23.20 10.03 6.61
run 3: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s
  load 23.20 10.03 6.61
run 4: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
  load 25.51 10.73 6.85
run 5: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.84s
  load 25.51 10.73 6.85
run 6: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s
  load 27.55 11.40 7.09
run 7: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
  load 27.55 11.40 7.09
run 8: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.08s
  load 29.91 12.15 7.36
run 9: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
  load 29.91 12.15 7.36
run 10: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.95s
  load 31.44 12.77 7.58
run 11: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s
  load 31.44 12.77 7.58
run 12: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
  load 32.84 13.37 7.81
run 13: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s
  load 32.84 13.37 7.81
run 14: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
  load 34.14 13.96 8.03
run 15: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s
  load 34.14 13.96 8.03
run 16: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
  load 35.33 14.54 8.25
run 17: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
  load 35.33 14.54 8.25
run 18: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s
  load 36.50 15.13 8.47
run 19: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
  load 37.58 15.71 8.70
run 20: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.96s
  load 37.58 15.71 8.70
```

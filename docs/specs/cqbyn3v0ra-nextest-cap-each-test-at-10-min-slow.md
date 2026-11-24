---
task_uid: "01m843trwpx1e7mwcqbyn3v0ra"
task: "T2105"
rev: 2
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: ".config/nextest.toml [profile.default] sets slow-timeout = { period = \"60s\", terminate-after = 10 } with a comment giving the reason (a vps01 run waited 7h45m on a self-deadlocked test); [profile.ci] does not set slow-timeout, so the ci profile inherits it"
    required: true
    manual: false
  - id: "C2"
    text: "One new [[profile.default.overrides]] sets slow-timeout = { period = \"60s\", terminate-after = 30 } for exactly the tests in [profile.ci] default-filter (same package(...) and test(=...) terms) plus package(fe-syntax) and test(=every_extension_highlights_its_sample), with a comment saying to keep it in step with the slow set; the existing default-filter, retries override, fsevents test group, store, junit and setup scripts are unchanged"
    required: true
    manual: false
  - id: "C3"
    text: "TESTING.md Slow tests paragraph states the 10-minute per-test cap, the 30-minute cap for the slow set, and that a test added to the slow set is added to both filtersets in .config/nextest.toml"
    required: true
    manual: false
  - id: "C4"
    text: "Evidence in the decision file: a scratch cargo crate outside the repo, using the new file's [profile.default], [profile.ci] and override slow-timeout lines, has a test that locks one std::sync::Mutex twice; cargo nextest run --profile ci reports it TIMEOUT and exits nonzero within 11 minutes (command and output recorded)"
    required: true
    manual: false
  - id: "C5"
    text: "The gate's full-workspace test stage passes with no TIMEOUT result, and the decision file records the longest test's name, its duration (under 600 s) and the gate run id, read from the gate's own test-stage record (~/.local/state/regatta/runs/\u003crun id\u003e/test.fastedit.log or test.fastedit.report.json), because rgt deletes the gate snapshot and its target/nextest/ci/junit.xml after the run"
    required: true
    manual: false
---

# nextest: cap each test at 10 min (slow-timeout 60s x 10) so a hung test fails instead of hanging the run; 30 min for the slow set

`.config/nextest.toml` sets no slow-timeout or terminate-after, so a hung test blocks a run forever. On vps01 a test locked a std::Mutex twice inside an assert_eq! failure message, and the run waited 7h45m. Fix: in [profile.default] (ci inherits it), set slow-timeout = { period = "60s", terminate-after = 10 }, so a hang becomes a 10-minute TIMEOUT failure (SIGTERM, then a kill after the grace period). Slow-set tests can take minutes under parallel load in post_merge's test_all (the fe-vim decision file records 103 s alone and "minutes under load"), and the whole-grammar wasm_parity run has no recorded duration. Give those 30 minutes with a [[profile.default.overrides]] entry that repeats the slow-set terms, because an override can't refer to default-filter. Nextest checks profile.default.overrides for --profile ci after ci's own overrides, one setting at a time, so the FSEvents retries override is unaffected. No production code and no regression test. Because .config/ changed, the gate tests the whole workspace. Requested by the vps01 lead (v8 / T2094 report, 11-23).

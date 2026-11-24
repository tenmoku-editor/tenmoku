# T2105: nextest slow-timeout

`.config/nextest.toml` now sets `slow-timeout = { period = "60s", terminate-after = 10 }`
in `[profile.default]` (`[profile.ci]` inherits it), and 30 periods for the slow set
through one `[[profile.default.overrides]]` entry.

## C4: a hung test times out

Scratch crate outside the repo (`scratchpad/hang`, one test that locks a
`std::sync::Mutex` twice), with the new file's `[profile.default]` slow-timeout,
the override's slow-timeout (filter pointed at the scratch package, matching no
test) and a `[profile.ci]` with no slow-timeout:

    $ cargo nextest run --profile ci
     TERMINATING [>600.000s] (───) hang locks_twice
         TIMEOUT [ 600.029s] (1/1) hang locks_twice
         Summary [ 600.038s] 1 test run: 0 passed, 1 timed out, 0 skipped
    error: test run failed
    cargo nextest run --profile ci  0.68s user 0.57s system 0% cpu 10:01.20 total
    exit=100

## C5: longest test in the gate run

rgt deletes the gate snapshot and its `target/nextest/ci/junit.xml` after the
run, so the lead respecced C5 (revision 2) to take its evidence from the gate's
own test-stage record. This reads the per-test durations from that record: run
`01m85b0fw65ps2yc6r8dzv3kc7`, `~/.local/state/regatta/runs/01m85b0fw65ps2yc6r8dzv3kc7/test.fastedit.log`
(and `test.fastedit.report.json`).

- Summary: `12157 tests run: 12157 passed (1 slow), 65 skipped`, 618.034 s.
- No `TIMEOUT` line in the log.
- Longest test: `fe-workbench tests::debug_session::a_read_run_keeps_its_output_whatever_the_scrollback`
  at 82.721 s (next: 43.453 s, 29.472 s), under the 600 s cap.

# T752: a_running_game_gets_keys_and_is_unloaded_when_a_tick_overruns flake

The test had two separate flakes. Both are fixed in the test; only test code
changed (C2).

## 1. A removed panel queued an event (the gate failure)

The failing gate run (`01m8cfphf963kv981tfhavejfg`) panicked at the last check,
`events().queued(GAME_ID) == 0`, with 1 queued.

The test kept its own strong `Entity<PluginPanel>`. Unloading the plugin
forgets its event queue and takes the panel out of the dock, but the held panel
was never released, so its 33 ms poll kept running. The poll found the panel not
shown and queued `Hidden`, which made a new queue for the unloaded plugin. It
fired only when the plugin thread's stop reached the main thread during one of
the test's clock advances, before the advance ran the poll.

The test now drops its handle once the panel ticks, so the dock alone holds it,
as in the app. It also advances past one more poll before the last check. With
that advance and without the drop, the test fails every time with the gate's
assertion.

## 2. The 100 ms call deadline stopped the plugin early (found under load)

With only fix 1, run 10 of 20 timed out waiting for the plugin to run: it was
`Stopped` with "a call ran past its 100ms deadline" while loading. The test ran
every call under the real deadline. It now uses 2 s, as
`plugin_host_tests` does for the same reason. The spinning tick still overruns
it and is what the test expects to stop the plugin. The 100 ms figure itself is
held by `fe-plugin`'s `each_call_still_has_the_100ms_deadline`.

## Evidence (C1)

Linux, 24 cores. Three `cargo nextest run -p fe-workbench --no-fail-fast` suites
(unit and integration tests, 3941 each) ran at once, and
`cargo test -p fe-workbench --lib -- --exact <test>` ran 20 times in a row beside
them, all three suites still running at each start.

| Round | Code | Result | 1-minute load |
|---|---|---|---|
| 1 | fix 1 only | 19/20 (run 10, cause 2) | 33–64 |
| 2 | both fixes | 20/20 | 33–65 |

The test also passed inside all three round 2 suites. Those suites each had at
most one failure, both in `spell_ranges_tests`, which this change doesn't touch:
`markdown_asks_for_its_grammars_when_they_are_not_compiled` and
`completeness_covers_what_was_asked_past_the_rows` (the latter also failed once
in round 1).

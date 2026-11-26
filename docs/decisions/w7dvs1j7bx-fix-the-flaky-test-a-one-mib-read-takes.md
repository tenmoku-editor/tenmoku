# T751: flaky `a_one_mib_read_takes_one_round_trip`

## Cause
The test timed the best of three 1 MiB reads at 25 ms one way and required
less than 75 ms. A read takes about 54 ms on an idle machine, which left some
21 ms for 1 MiB to cross a socket and the spinning relay threads of the
delayed link. Under load all three reads can run over. Reproduced on the
unchanged test: 7 of 20 runs failed, at 77 to 84 ms.

## Fix (test code only, `crates/fe-remote-client/tests/remote_fs.rs`)
The test checks the round trip itself instead of timing it. The client's
writer sends the read's request and drops every later byte, so nothing else
from the client reaches the server. The read completes only if the server needs
nothing more: no second request, and no credit. It also checks that the client
did write something afterwards (its credit grants), so a writer that stopped
dropping would fail the test.

- No clock decides a pass. The two 60 s waits only end a hang.
- With the client's window temporarily set to 1 MiB, the test fails ("the read
  needs more than one round trip"), as the old 75 ms bound was meant to.
- A larger bound was rejected: scaling the latency up would still pass or fail
  on how the machine schedules.
- **No longer covered:** elapsed time. A delay that costs no round trip, such as
  a server that stalls before replying, would not fail this test. D § 11's
  50.7 ms is a latency figure; this test now holds only its "one round trip".
- This replaces the description of the test under "Tests" in
  `nbh5y4a0v3-remote-host.md` (best of three, 75 ms).

## Evidence
On a 24-core Linux machine, at commit 358669047, with the `remote_fs` test
binary from `cargo test -p fe-remote-client --test remote_fs`. (The spec's
`go test -count=20` does not apply to this Rust repository.) Load was busy-loop
processes plus loops that ran the whole `remote_fs` suite concurrently, while
the one test ran consecutively with `--exact`.

| Test    | Busy loops | Suite loops | One test, consecutive | Suite runs               |
|---------|-----------:|------------:|-----------------------|--------------------------|
| Old     |         24 |           4 | 13 passed, 7 failed   | 57 passed, 37 failed     |
| Fixed   |         24 |           4 | **20 passed of 20**   | 66 passed, 0 failed      |
| Fixed   |         48 |           8 | 500 passed of 500     | 1511 passed, 0 failed    |

The 1-minute load average was about 27 during the 20-run rows and reached 181 by
the end of the 500-run row. The old test's kept failing suite logs (the last of
each loop) each failed only on this test.

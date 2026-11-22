---
task_uid: "01m8231ys5ge2qphmk93sccpw5"
task: "T438"
rev: 1
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "The bulk-window throughput assertion no longer depends on wall-clock rate under host load, and still fails if the bulk window starves the capped link"
    required: true
    manual: false
  - id: "C2"
    text: "cargo nextest run -p fe-remote-proto --test mux_bulk_window passes 20 times in a row while the host runs a parallel cargo build (stress-ng or a second nextest)"
    required: true
    manual: false
---

# Flaky under load: fe-remote-proto mux_bulk_window a_capped_link_sizes_a_bulk_window_to_twice_its_bdp_and_stays_full (wall-clock rate under 0.6x cap) (Mac T2099)

`fe-remote-proto::mux_bulk_window a_capped_link_sizes_a_bulk_window_to_twice_its_bdp_and_stays_full`
failed the Mac post-merge check of main 54b0280fa (run 01m821899qrhnw4vs0e4z4ens6, load ~20) with
`4.6 MiB/s` at tests/mux_bulk_window.rs:155: the delivered rate over the last 2 s of a 5 s drink
fell under `0.6 * cap` (4.8 MiB/s on an 8 MiB/s simulated link). It passed in about 40 earlier runs.
The rate is wall-clock bytes over elapsed time, so a loaded host that starves the reader or the
simulated link's threads lowers it without any window bug. The window-size assertions above it held.

Make the throughput check robust to host load without losing what it proves (twice the BDP keeps
the link busy): for example compare against what the same simulated link delivers to a
non-bulk stream in the same run, or measure the link's own busy time rather than wall-clock, or
assert only that the window never starves the link (in-flight bytes stay at the window). Apply the
same treatment to `a_capped_link_delivers_at_its_cap` if it shares the weakness. Test-only change.

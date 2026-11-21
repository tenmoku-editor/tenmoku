---
task_uid: "01m7yxfrfhwn50y5pmz5bpy500"
task: "T404"
rev: 4
type: "chore"
complexity: "xs"
criteria: []
---

# fe-remote-proto: mux_credits stalled-channel test races the cancel against in-flight data (T395 flake)

mux_credits::a_stalled_channel_does_not_block_another_and_cancel_stops_within_its_window failed in the neon sync gate under load (run 01m7yqb9796rew16scamyjj465): `only 983100 of 1310720 in flight` at tests/mux_credits.rs:183.

The test calls `bulk.cancel()` right after the server reports Full, then asserts every byte of the window (minus one chunk) was received. Under load the client has not yet received all in-flight data when it cancels, so bytes received can be below the window. Fix the test (not the protocol): wait until the client has received window - chunk bytes (drain with recv before cancel, bounded by a timeout) or assert only the upper bound plus that the credit returns. No sleeps. Run the test 50 times under `cargo nextest run --stress-count` or a loop to show it stable.

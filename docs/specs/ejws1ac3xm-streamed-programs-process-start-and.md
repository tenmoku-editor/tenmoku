---
task_uid: "01m71szax9ap24z0ejws1ac3xm"
task: "T1703"
rev: 3
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "`process.start` and the `child` resource exist for background jobs only; `start` called from an interactive call is refused with a typed error."
    required: true
    manual: false
  - id: "C2"
    text: "Per-stream backpressure of 1 MiB: a fast writer blocks while the guest does not read; at most 8 live children per job (host-capped); children are killed as a tree (P2's ProcessTree) on cancel and at the job's wall time."
    required: true
    manual: false
  - id: "C3"
    text: "New tests in kiln/crates/kiln-plugin/tests/process_stream.rs: `stream_fast_writer_blocks_until_guest_reads`, `stream_cancel_kills_grandchildren`, `stream_wall_time_kills_the_tree`, `stream_start_refused_in_interactive_call`, `stream_ninth_child_refused`, with Windows twins using the P4 script helper."
    required: true
    manual: false
  - id: "C4"
    text: "Only granted programs (`process.spawn:\u003cname\u003e`) can start; environment per P3. `start`'s `cwd` is checked against the pinned roots at start and again just before the launch; it is a convenience, not a security boundary (the granted program runs unsandboxed with the user's rights), and two windows are a known limit recorded in the spec and `process.wit`: a root retargeted between its pin and the WASI preopen, and a directory replaced between the last check and the launch."
    required: true
    manual: false
---

# Streamed programs: process.start and child, background only, backpressure, tree kill (B4, 3.3)

# Streamed programs: process.start and child, background only, backpressure, tree kill (B4, 3.3)

Draft id B4 of the plugins stage 3 plan (docs/decisions/dqsjfernkt-plugins-stage-3.md, section 6). The design is approved by the user (2026-11-09); questions Q1 to Q7 follow the design's proposals.

Section 3.3. Depends on B7 only to serialise edits to src/lib.rs and wit/process.wit scope neighbours.

Read the design sections named above before starting. Do not widen --scope; if a file outside it is needed, stop and say so.

## Known limits (rev 3, lead decision 2026-11-21)

`start`'s `cwd` is a convenience checked at start, not a security boundary. The granted program (`process.spawn:<name>`) runs unsandboxed with the user's rights and can change directory anywhere itself, so a `cwd` outside the plugin's roots gives the plugin no authority beyond its grant. `start` checks the `cwd` against the roots, pinned when the store is made, and checks it again just before the launch. Two windows are known and accepted for this card:

1. **Root open vs WASI preopen.** The roots that `start` checks against are opened apart from the WASI preopens (`kiln-plugin/src/fs.rs` `configure` opens each root by path again). A root retargeted between the two opens can name a different directory for `start` than WASI holds, or stay a root for `start` when its preopen failed.
2. **Check then launch.** The program starts in its directory by path (`Command::current_dir`). A directory replaced between the last check and the launch can start it elsewhere.

Both windows are recorded in `wit/process.wit` (`start`) and in `src/process_stream.rs`'s module docs. Closing them is a follow-up card: a handle-based preopen in `kiln-plugin/src/fs.rs` shared with `start`, plus a launch into an open directory (`fchdir` before exec) in `kiln-core/src/platform/process_tree.rs` on Unix. On Windows it stays a documented limit, since `CreateProcess` takes the directory as a path. Neither file is in this card's scope.

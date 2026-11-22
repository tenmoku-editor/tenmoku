---
task_uid: "01m81h98agcdvhkgnbh5y4a0v3"
task: "T411"
rev: 1
type: "feature"
complexity: "l"
criteria:
  - id: "C1"
    text: "tests/remote_fs.rs runs an in-process tenmoku-server session over the `Delayed` transport: read, stat, list_dir, canonicalize and the tree operations match LocalFs on the same fixture, and write_atomic conflicts come back as Conflict{current} for Matches and Absent"
    required: true
    manual: false
  - id: "C2"
    text: "`a_one_mib_read_takes_one_round_trip` passes at 25 ms one way with the 1.25 MiB window (under 2 x RTT plus slack)"
    required: true
    manual: false
  - id: "C3"
    text: "A read in flight when the link drops is retried once on the next connection and a write is not retried (End(LinkLost) surfaces as an error)"
    required: true
    manual: false
  - id: "C4"
    text: "tests/remote_watch_search.rs: a watch delivers host-side changes as FsChange batches; Quick Open's file list arrives in streamed batches with the first before the walk ends; dropping a search stream sends Cancel"
    required: true
    manual: false
---

# fe-remote-client RemoteHost: FsHost, WatchHost and SearchHost over the connection, reads retried once on a lost link, writes never, streams cancelled on drop (remote 4d) (Mac T2050)

# RemoteHost for fs, watch and search (remote 4d)

D § 5 (HostSet chosen from the location: `RemoteHost(conn)`), § 7 ("On resume": reads retried once, writes never), § 11 (nothing here blocks: every call is async with a pending state), § 12 (measured costs to compare against).
Uses the fe-host traits from cards 1a/1a-tree/1b and the server services from 3b. No UI here.

## Rules
Read the decision sections named above in `docs/decisions/j8wvgszah0-remote-development.md` (and the design file `docs/designs/remote-development.md` where cited) before starting; they are the spec, this card only cuts a slice of them. Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing (D § 12 spike notes). Gate tests on the affected crates only. Not here: anything the decision assigns to another phase (D § 13).

## Implementation notes

The choices are in `docs/decisions/nbh5y4a0v3-remote-host.md`. In short:

- `RemoteHost(conn)` holds a `Conn`, a slot for the connection now.
  Whoever reconnects the window replaces it (`reconnected`) or gives up on
  it (`disconnected`). Reads wait for the next connection and run once
  more. Writes fail with `LinkError::Lost` (`ConnectionAborted`).
- A watch watches again on each next connection, delivering a `Rescan`. A
  search or file list runs once more only if it had delivered nothing.
- C2's bound is under 2 × RTT (one RTT plus half of another). A 1 MiB
  window measures about 2 × RTT, which would pass "2 × RTT plus slack".

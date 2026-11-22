---
task_uid: "01m7zg4a2gh3vfvgmzexq7f2s0"
task: "T405"
rev: 3
type: "feature"
complexity: "l"
criteria:
  - id: "C1"
    text: "tests/fs_service.rs: read returns bytes and the host's DiskStamp in 64 KiB chunks; write_atomic honours every WriteExpect case through fe-host's LocalFs (a Matches with a stale stamp and an Absent onto an existing path end with Conflict{current} and leave the file unchanged); stat, list_dir, canonicalize and the tree operations answer as fe-project does locally"
    required: true
    manual: false
  - id: "C2"
    text: "tests/watch_service.rs: a watch request stays open and streams FsChange batches (Path and Rescan, the wire type unchanged); cancel ends it with End(Cancelled)"
    required: true
    manual: false
  - id: "C3"
    text: "tests/search_service.rs: a search streams FileMatches and a files request streams the walk in batches of 512 entries; cancelling after the first batch ends the request and the data after it stays within the window"
    required: true
    manual: false
---

# tenmoku-server services: fs (read, stat, list, write_atomic with WriteExpect, tree operations), watch and search over channels 1-3 with cancel (remote 3b) (Mac T2046)

# tenmoku-server fs, watch and search services (remote 3b)

D § 2.2 (fixed service channels 1 fs, 2 watch, 3 search; subscriptions), § 5 (FsHost, WatchHost, SearchHost: the server is the host side and calls the local impls), § 7 (WriteExpect, stamp checks before writing and before the rename, Conflict{current}), § 3 (what the server links: fe-project, fe-buffer's DiskStamp).
Paths are host paths (D § 6). The tree operations keep fe-project's beneath-root rules; whether plain reads outside the project root are allowed is not decided by D (the server runs as the user, D § 9): allow them unless the lead says otherwise, and note it in the lane decision file. The watch dirty set kept during the grace is T1991.

## Rules
Read the decision sections named above in `docs/decisions/j8wvgszah0-remote-development.md` (and the design file `docs/designs/remote-development.md` where cited) before starting; they are the spec, this card only cuts a slice of them. Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing (D § 12 spike notes). Gate tests on the affected crates only. Not here: anything the decision assigns to another phase (D § 13).

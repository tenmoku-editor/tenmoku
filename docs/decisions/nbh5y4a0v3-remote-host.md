# fe-remote-client: RemoteHost for fs, watch and search (T411, remote 4d)

These are the choices behind `crates/fe-remote-client/src/remote/` that
[`j8wvgszah0-remote-development.md`](j8wvgszah0-remote-development.md) ("D")
§ 5, § 7, § 11 and § 12 left open. The server side is T405's
([`mzexq7f2s0-tenmoku-server-services.md`](mzexq7f2s0-tenmoku-server-services.md)),
and the connect T397's ([`xqt03r6jc8-remote-client-connect.md`](xqt03r6jc8-remote-client-connect.md)).

## The connection a host calls over

- **`RemoteHost` holds a `Conn`, not a `Connection`.** A `Conn` is a slot
  holding the connection now. Whoever reconnects the window (phase 9's
  backoff) puts the next one in with `Conn::reconnected`. When the grace
  ends it calls `Conn::disconnected`, and from then on every call fails with
  `LinkError::Disconnected` (`NotConnected`). Each `Connection` put in
  counts as a new generation, so a call can tell whether the connection it
  lost has been replaced yet. Nothing in this card reconnects. Until phase 9
  does, a read that loses its link waits for a reconnect or a disconnect
  that only the window's owner can give.
- `Connection::over(mux, welcome)` makes a connection from a mux that was
  handshaken some other way. The tests use it for an in-process server.
  `Connection` now keeps the transport's `keep` in a mutex, so that it is
  `Sync` and an `Arc<Connection>` can be shared between threads.
- **`RemoteHost::host_set()`** is the window's `HostSet` (D § 5's
  `RemoteHost(conn)`). It has only `fs` for now, as `HostSet` does. The
  watch and search traits are called on the `RemoteHost` itself.

## Reads once more, writes never (D § 7)

- **Each call is one request.** If it gets `End(LinkLost)`, or the request
  can't be sent because the link has already gone, the call is a read or a
  write:
  - **Reads** are `Read`, `Stat`, `ListDir`, `Canonicalize`, `ListTree`,
    `PasteName`, `Exists`, `InTrash` and `RestoresFromTrash`. A read waits for
    the next connection and runs once more from the start; whatever the lost
    run had taken is dropped. If that run loses its link too, the read fails
    with `LinkError::Lost` (`ConnectionAborted`).
  - **Writes** are `WriteAtomic`, `CreateRecorded`, `Rename`,
    `DuplicateRecorded`, `CopyIntoRecorded`, `MoveInto`, `TrashRecorded`,
    `Restore` and `Truncate`. A write fails at once with `LinkError::Lost`,
    even when the upload was cut part way. Its outcome isn't known (the
    server may have committed it), so phase 9 re-stats, and its queued
    saves keep their `WriteExpect`.
- `LinkError::of(&io::Error)` finds the inner error, the way fe-host's
  `PathError::of` does, so a caller can tell the link's failure from the
  host's: "Not loaded — reconnecting", *save pending* (D § 11).
  `restores_from_trash` returns `false` when the host can't be asked,
  because it has no error to give.
- A `Status::Cancelled` reply (no call here cancels one) fails as
  `Interrupted`.
- If the host sends a body that its call doesn't answer with, the call
  fails with `InvalidData`. A read whose bytes don't add up to its `len`
  fails the same way.

## Streams

- **A watch** sends `Watch` and returns once the server's first, empty
  batch has come. That is the point at which the local impl's watcher is in
  place, so a change made after `watch` returns is reported, and a watch
  that can't start fails at once with the host's error. A thread then feeds
  the subscription. It polls the reply with a waker, and the subscription's
  source wakes it when dropped, so the request is dropped (sending `Cancel`)
  even while the host is quiet.
  - **When the link drops, the watch watches again on each next
    connection** and delivers `[FsChange::Rescan]` once the new watch is in
    place, because changes during the gap aren't known. Phase 9's resume
    replays the session's dirty set instead (D § 7). The watch ends if the
    window is disconnected, or if the host refuses the new watch (its root
    is gone, say).
- **A search or a file list** is a host `Stream` fed by a thread. Its items
  are what came over the link in one go: a frame, plus at most 15 more of
  the frames already waiting behind it. The cap keeps a walk that never
  pauses streaming: without it, frames that always had another waiting
  behind them would never be delivered. The server splits a file with more matches than one
  frame holds into consecutive entries of the same path (T405). When those
  pieces come together they are joined back into one entry, and a split
  line's ranges are joined onto that line.
  - **Known limit:** if the second piece of a split file arrives after the
    first has been delivered, the same path shows up in two batches. Only
    the protocol can mark a continuation for certain (a `more` flag on
    `Matches`, say), and fe-remote-proto is outside this card's scope. That
    needs a file with over 64 KiB of matches, split by a frame boundary that
    a stall then holds apart.
  - **Dropping the stream cancels the request.** A host `Stream` gives its
    producer no signal when it is dropped. It only sets a flag that the
    producer's next `send` sees. So the thread checks the flag every 20 ms
    (`STOP_POLL`) while it waits on the link, and drops the request once the
    flag is set. That bounds how late a `Cancel` can be, and ties no
    behaviour to timing. Making it prompt needs a hook in fe-host's
    `stream.rs`, which is outside this card's scope; the cost now is one
    wakeup every 20 ms per open search. A watch has no such delay, because
    this crate makes its subscription's source.
  - A search or file list that loses its link **before delivering
    anything** runs once more on the next connection, like a read. One
    that has delivered something ends with `SearchFailed::Io(LinkError::Lost)`,
    because running it again would deliver duplicates. That ending is the
    search panel's "interrupted", and Quick Open keeps its last list (D § 11).
  - A search's `Errors` batches are put into its summary, and a
    `SearchData::Failed` becomes `SearchFailed::InvalidPattern` or
    `InvalidGlob`, as T405 intended. A stream cancelled from this end ends
    with a summary marked `cancelled`, carrying the files and matches it
    delivered, or with `FilesSummary { complete: false }`. That matches
    what the local impls end with.

## Tests

- The tests run `tenmoku_server::serve` in-process over a Unix socket pair,
  with the client's end behind `Delayed`. The parity tests use 2 ms one way
  and the round-trip test 25 ms. To cut a link, the test shuts down a clone
  of the client's socket, waits for the mux to see the loss, and then
  reconnects with a fresh server.
- **`a_one_mib_read_takes_one_round_trip`** asserts that the best of three
  reads takes at least one RTT and less than one RTT plus half of another
  (75 ms at 25 ms one way). Here one round trip measures about 54 ms. With a
  1 MiB window a read takes two round trips, about 101 ms, which a bound of
  "2 × RTT plus slack" would still pass. The bound is set under 2 × RTT so
  the test can tell one round trip from two. Best of three keeps a
  scheduling stall from failing it. 128 runs, in 16 loops in parallel on 12
  cores, all passed.
- The tree operations that change the disk run on twin fixtures, one
  through the remote host and one through fe-project's `LocalTreeFs`. The
  test compares their results (relative to each root) and the two disks
  afterwards. Read-only operations are compared on the same fixture.
  fe-project's `test-support` (a dev-dependency) sends the in-process
  server's trashes to a test trash.
- The retry tests use a server `FsHost` that holds its reads and writes of
  one path until the test releases them, so the test knows the request is
  in flight when it cuts the link. A read that failed rather than waited
  would show up as an error when the test joins it after reconnecting. The
  cancel tests use fake server hosts that report when the server stops
  them. They hold the `RemoteHost` and check afterwards that the link is
  still up, so the stop can only have come from the `Cancel`, not from the
  link closing.
- Watch changes are waited for with `recv_timeout(30 s)`. That is an upper
  bound on the watcher's latency, not a sleep.

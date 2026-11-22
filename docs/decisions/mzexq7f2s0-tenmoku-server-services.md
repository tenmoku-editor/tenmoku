# tenmoku-server: the fs, watch and search services (T405, remote 3b)

The choices behind channels 1–3 of `crates/tenmoku-server` that
[`j8wvgszah0-remote-development.md`](j8wvgszah0-remote-development.md) ("D")
§ 2.2, § 5 and § 7 left open. The daemon, sessions and grace are T396's
([`4y9cwyhysf-tenmoku-server.md`](4y9cwyhysf-tenmoku-server.md)); its
"Control only" section no longer holds, since these channels are served now.

## The wire mirrors fe-host

- **`FsOp` is `FsHost`, one variant per call,** the tree operations
  included (`ListTree`, `CreateRecorded`, `Rename`, `DuplicateRecorded`,
  `CopyIntoRecorded`, `PasteName`, `MoveInto`, `TrashRecorded`,
  `RestoresFromTrash`, `Restore`, `InTrash`, `Exists`, `Truncate`), and each
  `FsData` reply carries what the call returned. Phase 2 drew channel 1
  before fe-host's tree API existed (docs/decisions/5zajsn5anw-fs-host-tree.md),
  with a `Create`, `Rename`, `Remove` and `Trash` that had no root and no
  undo record, a `Stat { follow }` and a `ListDir { ignored }` that no
  `FsHost` call matches, and a `readonly` the host never reports. Those
  went. A client's `RemoteHost` (phase 4) is then one request per trait
  call, with nothing to translate. The scope grew by a respec to
  `fe-remote-proto/tests/body_roundtrip.rs` and `frame_garbage.rs`, whose
  strategies and seeds built the old shapes.
- `Stat` replies `Stat(None)` for nothing at the path, as `FsHost::stat`
  returns `Ok(None)`. `ListTree` sends the entries in `Tree` batches, then
  `Listed { id }`; a failure sends `Unlisted { lost }` and then its error,
  since `ListReply` carries both. `Restore` sends the record as it is now
  before its path or its error, since `FsHost::restore` moves the record
  on failure too. `Truncate` replies nothing but its `End(Ok)`.
- `ListTree` has no global excludes: they are the client's snapshot and
  never cross the wire (fe-host's `ListRequest` says so). The host's
  `LocalTreeFs` reads its own once, per session.
- **`SearchOp::Files` gained `excluded`,** as fe-host's `SearchFilter` has
  it for both calls. **`SearchData::Failed`** names an invalid pattern or
  glob before the `End(Error(InvalidInput))`, so a client can rebuild
  fe-host's `SearchFailed`. An I/O failure is the `End`'s alone.
- **Known limit:** an error crosses the wire as its `io::ErrorKind` and
  message (`IoError`). The typed inner errors fe-host and fe-project attach
  for the caller to word (`PathError`, `RequestError`, fe-project's
  `NameError`) don't. The messages are the host's, in English. Phase 4
  decides whether its wording needs them; `IoError` would grow a field.
- Conversions between the wire and fe-host's types live in
  `tenmoku-server/src/convert.rs`, as free functions, because both are
  foreign there and fe-remote-proto's `Cargo.toml` (not in this card's
  scope) has no fe-host dependency. Phase 4 needs the reverse ones; moving
  them into fe-remote-proto behind a dependency on fe-host (GPUI-free,
  small) is the natural home then.

## Paths

- **A plain call (`Read`, `WriteAtomic`, `Stat`, `ListDir`,
  `Canonicalize`) takes any absolute host path,** inside the project or
  not. D § 9 has the server run as the user, with trust a client policy,
  so the root is no security boundary here; refusing outside reads would
  only break opening `/etc/hosts` from a remote window. Lead: say if this
  should change.
- **A relative path is refused with `InvalidInput`** (D § 6: paths are host
  paths), rather than resolved against the daemon's working directory,
  which is `/`. A tree operation's `root` must be absolute too; its other
  paths may be relative to the root, and fe-project's rules keep them
  beneath it (a path that leaves is `InvalidInput`).
- The tree operations take the `root` the client sends, not the session's
  `Hello.location`. `FsHost` takes the root per call, and checking it
  against the session would add no protection for the same reason as
  above.

## Threads, cancel and the waker

- **fs requests run on a pool of at most 8 workers per session,** started
  as requests need them. A slow disk holds up only its workers, and a
  burst of tree listings doesn't start a thread each. A request whose
  worker can't start runs on the accept thread instead of going
  unanswered. When the link goes, requests still queued are dropped (no
  one waits for them).
- **A request that uploads (`WriteAtomic`, `CreateRecorded`) runs on a
  thread of its own, never in the pool** (review sck4f9 F1). Its bytes
  hold the channel's credit until they're read, and the channel's credit
  is shared by every request on it: queued behind 8 workers busy with
  half-uploaded writes, a few writes' first chunks could take the whole
  window, and the writes being served would never get the rest of their
  bytes. The upload threads are as many as the client's writes in flight.
- **A cancelled request never reaches the host** (F2): a request checks
  for its cancel before the host's call, and an upload again once its
  bytes are in, so a rename, trash or truncate cancelled while it waited
  for a worker changes nothing and ends `Cancelled`. A cancel that lands
  while the call runs can't undo it; the request still ends `Cancelled`,
  as the mux has a cancelled request end, and the client treats it as an
  outcome it doesn't know.
- **A watch or a search runs on a thread of its own, with a forwarder
  thread** that waits on the host's `Subscription` or `Stream` and puts
  results in a mailbox. The request's thread takes them out and sends
  them. It registers the reply's waker (`Stream::poll_recv`) before every
  wait, and the mux wakes it on a `Cancel` or a lost link, so it stops at
  once even while nothing comes from the host: a watch on a quiet tree, or
  a search between sparse matches.
- **Stopping the host's work needed two small fe-host additions** (a
  respec added `fe-host/src/stream.rs`): `Subscription::into_parts`, so
  the forwarder can wait on the items while the request's thread drops
  the source, which stops the watcher and ends the forwarder's wait; and
  `Stream::cancel_flag`, so the request's thread can cancel the host's
  search while the forwarder waits in `recv`. Without them the host's work
  would run on until its next result: forever, for a quiet watch, and
  until the end of the tree for a sparse search, one per keystroke in the
  search panel (D § 11).
- A request ended with `Cancelled` returns at once; the host's work stops
  soon after (fe-project checks its flag before each file and at each
  match).

## Reads and writes

- **`Read` calls `FsHost::read`, which reads the whole file,** then sends
  it as `File { stamp, len }` and 64 KiB `Bytes` chunks. The stamp is the
  host's, taken from the open file. Streaming from the file would hold
  less, but the local impl is the editor's own, and the editor holds the
  whole file anyway.
- **`CreateRecorded` uploads its contents** as `WriteAtomic` does (`len`,
  then `Bytes` chunks), rather than carrying them in the request, whose
  body is capped at 1 MiB (F5). It creates nothing until they're all in.
- **`WriteAtomic` takes its `len` uploaded bytes, then calls
  `FsHost::write_atomic`,** fe-host's `LocalFs` commit: the stamp check
  before writing and again before the rename, and the no-clobber commit
  for `Absent` and an `AsFound` that found nothing (D § 7). A conflict
  replies `Conflict { current }` and writes nothing. Up to 16 MiB is set
  aside up front, and past that the buffer grows as bytes arrive, so a
  `len` alone can't make the server allocate. An upload past its `len`,
  or of anything but `Bytes`, is refused with `InvalidData`; a cancel or
  a lost link before the last byte writes nothing. A cancel after the
  last byte may still commit, since the commit isn't interruptible; the
  client then sees `Cancelled` and re-stats, as after any write it can't
  confirm (D § 7: writes are never retried blindly).

## Batches

- Every `Data` body stays under 64 KiB (`CHUNK`): lists (`Entries`,
  `Tree`, `Trashed`, watch batches, `Files`) are sent in as many frames as
  that takes, sized with `fe_remote_proto::encoded_len`.
- **A file with more matches than fit in a frame comes as consecutive
  `FileMatches` of the same path,** its lines in order, and a line with
  more ranges than fit as consecutive entries of that line
  (`SearchData::Matches` says so). A client concatenates them. The room
  for lines is what the frame has left after that file's own encoded path
  and rel (F4): a 3 KiB path of bytes that aren't UTF-8 has a rel three
  times that long.
- **A summary's errors come before it, in `SearchData::Errors` batches,**
  and the `Summary` itself carries none (F3): a search of a tree with
  thousands of unreadable files has more errors than a frame holds. A
  client appends them to the summary. Each error's message, and the texts
  of `SearchData::Failed` and of the failed search's `End`, are cut to
  4 KiB on a char boundary: an invalid pattern's message quotes the
  pattern, which may be as long as a request.
- **`Files` sends whole batches of 512** (D § 12's walk batches): fe-project's
  walk hands over batches as its threads fill them (a directory's worth
  here and there), so what doesn't make up 512 waits for the host's next
  batch, and the rest goes when the walk ends. The wait is at most the
  walk's next batch, and a walk under 512 files sends its one batch when
  it ends.
- **The watch.** The first `Data` is an empty batch, sent once
  `WatchHost::watch` has returned, so a client (and a test) knows a change
  from then on is reported. Batches go as the host made them while the
  client keeps up. `WatchHost` never holds its watcher up, so while the
  client's credit is spent the host's batches merge into one waiting
  batch, and past 10,000 changes that batch becomes a single `Rescan`,
  which covers whatever else changes until it is sent. The dirty set a
  session keeps during its grace is T1991's.

## Hosts and tests

- `Config.hosts` makes each session's `Hosts` (`fs`, `watch`, `search`),
  fe-project's `LocalTreeFs`, `LocalWatch` and `LocalSearch` by default. A
  test gives its own: the watch tests feed a fake `WatchHost` through a
  rendezvous channel, so the test knows the server has taken each batch
  (the merge-into-`Rescan` test depends on it), and a fake `SearchHost`
  that finds nothing after its first batch until it is cancelled shows the
  cancel reaches the host.
- The long non-UTF-8 path test reports its file from a fake `SearchHost`
  (review 5wx7w5 F6): APFS refuses names that aren't UTF-8 and Darwin's
  paths stop at 1024 bytes, so no real file could carry it on macOS.
- The fs tests compare each reply with what `LocalTreeFs` answers here,
  or, for an operation that changes the disk, with the disk afterwards.
  fe-project's `test-support` feature (a dev-dependency) sends their
  trashes to a test trash of the process's own.
- The cancel tests use the smallest window (256 KiB) and fixtures of about
  four windows, read the first batch, cancel, and check the `End` is
  `Cancelled` and the data bytes after the cancel are at most the window.
- No test sleeps. The fake search spins on `yield_now` until its cancel
  flag is set.

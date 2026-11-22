# fe-remote-client: connect (T397, remote 4a)

These are the choices behind `crates/fe-remote-client` that
[`j8wvgszah0-remote-development.md`](j8wvgszah0-remote-development.md) ("D")
§ 1, § 2.5 and § 4 and the design's § 2.2–§ 3 left open. The protocol is in
[`7142g56xt6-remote-proto-mux.md`](7142g56xt6-remote-proto-mux.md).

## The transport

- **`RemoteTransport` is D's trait as written.** `Link` has D's `reader`,
  `writer` and `child: Child`, and one more field, `keep`. It holds what the
  transport must keep alive for as long as the link. For ssh that is the
  askpass session: its socket answers prompts and its folder is removed
  when it drops. `Link::into_proto` gives fe-remote-proto's link (whose
  `child` is an `Option`) for `Mux::client`.
- **fe-git's askpass `Session` is now public** (respec of T397).
  `Askpass::new` is unchanged. `Session::start`, `script`, `started` and
  `cancelled` are what fe-git's own remote commands use. ssh leads a process
  group of its own, so a cancelled prompt stops it the way it stops git.
  If the session can't be set up, the spawn fails rather than run ssh
  without it. Otherwise ssh could prompt, unseen, on a terminal the app
  happens to have.
- **The connect makes the `Askpass`** from the caller's helper, folder and
  answer function. It wraps the answer function so it can report
  `State::SigningIn` and notice a cancel. `SigningIn` lasts until the next
  state. No `Connecting` is re-sent after the answer.
- **The destination is passed to ssh verbatim.** One that starts with `-`
  is refused, since ssh would read it as an option. Turning `host:port`
  into `ssh://host:port` or `-p` is the connect dialog's job.
- **Remote words.** ssh joins the command's words with spaces for the
  login shell, so `remote_word` single-quotes any word that isn't plain.
  The bootstrap contains no single quote and no `\\`, so sh, bash, zsh and
  fish all read `'<script>'` the same way. csh and tcsh don't take a
  newline inside quotes, so a host whose login shell is csh is a known
  limit.

## The bootstrap

- **The script is `src/bootstrap.sh`, after one line of parameters:**
  `B=<install folder> F=<0|1>`. The folder name is checked to be a safe
  name (letters, digits and `.+-_`), so the argv is exactly
  `sh -c '<script>'` with nothing after it. It comes to 49 lines.
- **Tools.** It uses D's list plus `rm`, which it needs to drop a bad
  upload and to prune. GNU tar also runs `gzip` for `-z`. It also runs the
  installed server's own `--version`. C3's list leaves out `rm` and
  `gzip`, so review round 1 (F7) asked for a respec. Neither can go:
  pruning is removal, and decompressing on the client would triple the
  upload.
  `tests/bootstrap_tools.rs` runs an install and a prune with PATH holding
  only those tools: once with `sha256sum` and once with `shasum`.
- **The archive is a `.tar.gz` with `tenmoku-server` at its root.** The
  release card's `release.yml` must pack it that way. A dev server is
  packed the same way by the client (`pack`), so the script has one install
  path. The install's SHA-256 is of the uploaded bytes. A dev install's
  folder name is from the binary's SHA-256 (D § 4).
- **Failures are one `TENMOKU-BOOT error <message>` line on stdout**, so
  the client can show "Couldn't install on neon: <message>" with the tool's
  own words (mkdir's "Permission denied", head's "No space left on
  device").
- **Lines before the probe are skipped.** A login shell's greeting on stdout
  goes to the log as `host: …`, up to 256 lines. The client reads its lines
  one byte at a time, so nothing after them is taken from the pipe.
- **Unsupported hosts.** The client maps `uname` to the asset (Linux
  x86_64/amd64, Linux aarch64/arm64, Darwin arm64/aarch64) right after the
  probe, even when a server is present. Anything else ends the connect.
  Darwin x86_64 is unsupported, as D § 14 says.

## Pruning

- **"Newest" is by the server binary's modification time.** `tar -m` sets
  it to the time of install, and the folder just installed is never pruned.
  A tie goes to the later name in the glob's order, so the order is total,
  even on a filesystem (or a `test -nt`) with whole seconds.
- **An install whose daemon lock file is there is kept, held or not.**
  POSIX sh has no `flock`, the util-linux tool isn't on macOS, and nothing
  else sh can see shows a lock free:
  - `Daemon::claim`, and the attach that starts a daemon, take the lock
    before the pid in it is rewritten. A lock with a dead pid may be held.
  - Round 2 tried matching `/proc/*/fd/*` against the lock with
    `test -ef`. Review round 3 (F1) showed that it misses a lock taken
    after the glob is expanded, and one held by a process whose
    descriptors can't be read.

  A lock file goes when its runtime folder does, and then its install can
  be pruned. `$XDG_RUNTIME_DIR` is cleared once the user has no session,
  and `/tmp/tenmoku-server-<uid>/` at a reboot. HOME's fallback run folder
  sits inside the install and is never cleared, so an install whose daemon
  ran there stays (a known limit, below).
  It looks wherever the daemon may have
  put its lock: `$XDG_RUNTIME_DIR/tenmoku/`, HOME's fallback
  `~/.cache/tenmoku-server/<build>/run/` (the server's `Paths` uses HOME
  there, not `XDG_CACHE_HOME`) and `/tmp/tenmoku-server-*/`.
- **Each install writes a `build` file** with the build the installed
  server reports (`tenmoku-server --version`), as that is what names its
  daemon's lock. A dev install's folder is `dev-<sha>`, and a mismatched
  dev server's daemon runs under its own build, not the client's. An
  install with no `build` file (`--version` failed, or it came from
  elsewhere) is never pruned: its lock can't be found.

## The connect

- **States.** `Connecting`, `SigningIn{prompt}`, `Probing(Probe)`,
  `Uploading{sent,total}` (at 0 and after each 64 KiB),
  `Reinstalling{theirs}`, `Starting` and `Connected`. Downloading and
  Verifying come from the next card, before the upload.
- **The reinstall.** It happens once, after a `Refused` (the server's check)
  or a `Mismatch` (the client's). It sends `F=1`, which uploads even though
  a server is present, and the `mv` replaces the stale binary. `theirs` is
  read from the refusal's reason (`… server protocol N build "X"`), or is
  the whole reason when that isn't there. Other handshake failures
  (closed, garbage) are errors without a reinstall. A dev server is never
  reinstalled, since its folder is named for its own bytes, so a second
  upload would send the same binary. It ends with D § 4's message instead.
- **Dev builds.** `ServerSource::for_build` uses `remote.dev_server` when
  it is set. A `-dirty` build without one is `NoRelease`. It fails after
  the probe with the design's message, which names the host's asset. It
  fails even when a server is installed for its build id, since another
  checkout's changes can have the same id. A clean but unreleased commit is
  found out at the download (next card).
- **Errors are the design's § 2.4 table, in English.** ssh's failures are
  told apart by its stderr: "REMOTE HOST IDENTIFICATION HAS CHANGED" means
  the host key changed. "Permission denied (", "Too many authentication
  failures" or "Host key verification failed" means the sign-in wasn't
  accepted, and so does a cancelled prompt. Anything else gives ssh's last
  stderr line. Translating the messages belongs to the UI card (phase 5).
- **The connection log** is ssh's stderr plus a `bootstrap:` transcript,
  kept to the last 2000 lines. It is in `Failed::log` and
  `Connection::log`. ssh's stderr keeps going into the log after the
  connect. stderr is read as bytes and decoded lossily, line by line, so a
  path that isn't UTF-8 doesn't end the log. A failed attempt waits, for at
  most 2 s, for the copying thread to reach stderr's end before the log is
  taken. A remote that ended by itself (an error line, or EOF) is waited
  for, not killed, so ssh's last lines get written. The test for the wait
  checks that a failing server's last line, after a thousand lines of
  noise, is in the log. It can't make the thread lose the race without a
  hook in the crate, so it passes without the wait too.

## Known limits

- The handshake has no timeout of its own. A server that execs but never
  answers holds the connect, since ssh's keepalive only notices a dead
  path. A connect-level deadline goes with the dialog's Cancel (phase 5).
- A csh or tcsh login shell on the host (see "Remote words").
- Hosts with no `$XDG_RUNTIME_DIR` (macOS, and Linux without
  pam_systemd) keep every install whose daemon has run, since its lock
  file lives in the install. On every host a lock first created while a
  prune runs (an old client connecting at that moment) can lose its
  install. The daemon's keeper puts its socket and lock back, and the old
  client's next connect reinstalls. The exact fix is a pruner that can
  `flock`: tenmoku-server taking each old build's lock, and deleting it
  while it holds the lock. That changes tenmoku-server, so it is a
  follow-up card.

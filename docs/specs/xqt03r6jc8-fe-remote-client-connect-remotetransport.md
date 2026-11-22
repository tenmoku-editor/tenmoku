---
task_uid: "01m7xbb6fve76avnxqt03r6jc8"
task: "T397"
rev: 4
type: "feature"
complexity: "l"
criteria:
  - id: "C1"
    text: "`RemoteTransport`/`Link` are D § 1's shape; SshTransport runs `ssh -T -o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o ConnectTimeout=15 \u003cdest\u003e -- sh -c '\u003cscript\u003e'`, never BatchMode or ControlMaster, with SSH_ASKPASS from fe-git's `Askpass::new` and SSH_ASKPASS_REQUIRE=force (tests/ssh_argv.rs, with a recording fake ssh on PATH)"
    required: true
    manual: false
  - id: "C2"
    text: "tests/bootstrap.rs runs the bootstrap under `sh` with a temporary HOME and XDG_CACHE_HOME: missing -\u003e `TENMOKU-BOOT 1 \u003cos\u003e \u003carch\u003e missing`, upload of `\u003csize\u003e \u003csha256\u003e` then the bytes, host-side SHA-256 check, unpack, chmod 755, rename, `TENMOKU-BOOT ready`, exec; present -\u003e ready with no upload; a wrong SHA-256 installs nothing; pruning keeps the newest three builds and skips one whose daemon lock is held"
    required: true
    manual: false
  - id: "C3"
    text: "The script is POSIX sh under 60 lines using only uname, mkdir, head -c, chmod, mv, tar (with gzip, which GNU tar execs for -z), rm (to prune old builds and drop a bad upload) and sha256sum or `shasum -a 256`; `uname -m` x86_64/amd64 and aarch64/arm64 map to the asset names and anything else ends with \"tenmoku-server isn't built for \u003cos\u003e \u003carch\u003e\""
    required: true
    manual: false
  - id: "C4"
    text: "A dev_server binary installs to `~/.cache/tenmoku-server/dev-\u003csha256[..12]\u003e/` and a build mismatch at the handshake ends with D § 4's exact message; with no dev_server a dev build stops with \"This development build has no matching tenmoku-server release. Set remote.dev_server to a server built for linux-x86_64.\""
    required: true
    manual: false
  - id: "C5"
    text: "A `Connection` reports the design § 2.2 states in order (ssh starting, waiting on sign-in, probing, uploading with progress, starting, connected) and the handshake refusal triggers one reinstall, a second being an error (tests/connect_states.rs over a child-stdio transport and a fake server)"
    required: true
    manual: false
---

# fe-remote-client connect: RemoteTransport, SshTransport with askpass, the POSIX sh bootstrap that probes, uploads and execs the server, dev_server installs and the connection state events (remote 4a) (Mac T2047)

# fe-remote-client connect (remote 4a)

D § 1 (transport, options, askpass, the trait), § 4 (bootstrap, install directory, pruning, dev builds), § 2.5 (client-side build check). Design § 2.2 (Connecting states), § 2.4 (error table), § 3 (first connect).
The client crate is named `fe-remote-client` (D names no client crate; the parents and T1991/T1992 use this name). GPUI-free. The archive source is a parameter here (a path or bytes); downloading and verifying the release archive is the next card. Askpass: reuse `fe_git::askpass::Askpass::new` unchanged (`crates/fe-git/src/askpass.rs`); if it needs a change, stop and respec. The connection log (ssh stderr plus the bootstrap transcript) is kept for Show Log.

## Rules
Read the decision sections named above in `docs/decisions/j8wvgszah0-remote-development.md` (and the design file `docs/designs/remote-development.md` where cited) before starting; they are the spec, this card only cuts a slice of them. Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing (D § 12 spike notes). Gate tests on the affected crates only. Not here: anything the decision assigns to another phase (D § 13).

## Implementation notes

- The respec widened scope to `crates/fe-git/src/askpass.rs`, where `Session` and its `start`, `script`, `started` and `cancelled` are now public. There is no behaviour change, and `Askpass::new` is unchanged.
- The choices are in `docs/decisions/xqt03r6jc8-remote-client-connect.md`. The main ones:
  - `Link` has one field beyond D's, `keep`, which holds the askpass session.
  - The bootstrap also needs `rm`, and `gzip` for GNU tar.
  - Pruning tests whether a daemon is alive by the pid in its lock and `kill -0`, since sh has no `flock`. An install whose daemon lock file exists (held or not, since sh can't tell) and one whose build is unknown are kept. Exact, flock-based pruning in tenmoku-server is a follow-up card.
  - Each install records the build its server reports, in a `build` file.
  - The archive has `tenmoku-server` at its root, which the release card must match.

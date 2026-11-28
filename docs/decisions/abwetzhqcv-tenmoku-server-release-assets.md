# tenmoku-server release assets (T2049, remote 4c)

Lane notes and choices for the slice of D (`docs/decisions/j8wvgszah0-remote-development.md`)
§ 3, § 4 and § 14 that T2049 builds.

## Real archive sizes (§ 14)

Built on 2026-11-27 from f29f58d20 (`tenmoku-server 0.3.0+f29f58d208c3`) with
`scripts/build-tenmoku-server.sh`. The Linux builds were cross-linked on macOS with rust-lld,
and needed the `renameat2` change below:

| Asset | Archive (.tar.gz) | Binary |
|---|---|---|
| linux-x86_64 | 1,935,259 B (1.8 MiB) | 4,357,056 B, `static-pie linked`, stripped |
| linux-aarch64 | 1,790,815 B (1.7 MiB) | 3,666,424 B, `statically linked`, stripped |
| macos-arm64 | 1,652,005 B (1.6 MiB) | 3,546,160 B |

So the 10–15 MB estimate in § 4 was high by almost ten times. At the 2.8–15.2 MiB/s measured to
neon, the first-connect upload takes about 0.1–0.7 s. The server so far has only the fs, watch and
search services (phase 3). Phases 6–8 add the process, pty and git services, and they will make
it bigger: fe-git's gix most of all. The CI smoke step prints the packed and unpacked sizes on
every run, so the numbers can be read again from any release run.

## Found: musl has no `renameat2` symbol

The musl that Rust bundles for `*-unknown-linux-musl` (the self-contained `libc.a`) defines
`renameat` but not `renameat2`. `fe_project::fs_ops::rename_exclusive`
(`crates/fe-project/src/fs_ops.rs:2537`) calls `libc::renameat2`, so both static Linux links fail
with `undefined symbol: renameat2`. Calling it as `libc::syscall(libc::SYS_renameat2, …) as c_int`
links on musl and on glibc. On glibc it is also what the libc wrapper does, and glibc only gained
that wrapper in 2.28. The lead added `fs_ops.rs` to the scope (rev 3), and the change is made
there. Both musl builds above were made with it, and rebuilt from the committed change.

## Choices

- **One job per asset, each on its own architecture's runner** (`ubuntu-24.04`,
  `ubuntu-24.04-arm`, `macos-latest`), so nothing is cross-linked in CI. Rust's musl targets link
  musl statically by default. `build-tenmoku-server.sh` checks the result with `file(1)` and
  refuses a Linux binary that is dynamically linked or names an interpreter, and any binary for
  another OS or architecture.
- **The archive** holds `tenmoku-server` at its root, mode 755, as the client's bootstrap
  (`crates/fe-remote-client/src/bootstrap.sh`) and `source::pack` expect. Beside it is
  `Licenses/` (LICENSE, NOTICE, Apache-2.0.txt, MIT.txt, THIRD_PARTY.md), because the binary
  links MIT and Apache crates statically. The bootstrap unpacks into a temp directory and moves
  only `tenmoku-server`, so the extra files are never installed. There is no per-archive .sha256,
  because the signed sums replace it (§ 4).
- **Signing in its own job, `tenmoku-server-sums`.** It needs all three archives. It reuses the
  `sign_update_feed` binary that the linux-x86_64 app job already builds, handed over as a
  one-day artifact (`sign_update_feed-linux-x86_64`). That name doesn't match `Tenmoku-*`, so the
  signer is never published. Building the example there would rebuild every one of the app's
  dependencies.
- **One artifact for publish.** The raw archives are `tenmoku-server-*` artifacts, which only the
  sums job downloads. The sums job uploads all five files as `Tenmoku-<v>-tenmoku-server`, so
  publish's `Tenmoku-*` download takes the archives and their sums together or not at all.
- **No key, no release.** A tag push, stable or preview, publishes, and a server whose sums are
  unsigned could never be installed, so the sums job runs `--sums --require-signature` there and
  fails without `TENMOKU_UPDATE_PRIVATE_KEY`. publish needs the sums job, so the tag publishes
  nothing. This is stricter than the Velopack feeds, which are left unsigned with a warning. A
  manual run (workflow_dispatch) leaves the sums unsigned and warns. Publish's stale-signature
  guard also covers `tenmoku-server-*-SHA256SUMS.sig`: a rerun that made no .sig, over a release
  that has one, is refused.
- **The smoke step** checks that `--version` prints exactly
  `tenmoku-server <version>+<git rev-parse --short=12 HEAD>`, which is the build the app jobs
  stamp from the same commit. A `-dirty` server could never pass the client's strict handshake.

## Workflow edit

This card changes `.github/workflows/release.yml`, so `rgt finish` needs a human risk approval.

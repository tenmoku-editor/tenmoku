---
task_uid: "01m7x6eszp0cfdbvabwetzhqcv"
task: "T2049"
rev: 3
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "Every tagged release, stable and preview, uploads `tenmoku-server-\u003cversion\u003e-linux-x86_64.tar.gz`, `-linux-aarch64.tar.gz`, `-macos-arm64.tar.gz`, `tenmoku-server-\u003cversion\u003e-SHA256SUMS` listing the three, and `tenmoku-server-\u003cversion\u003e-SHA256SUMS.sig` made with `examples/sign_update_feed.rs sign` and the TENMOKU_UPDATE_PRIVATE_KEY secret"
    required: true
    manual: false
  - id: "C2"
    text: "The Linux archives are static musl builds; scripts/build-tenmoku-server.sh builds and packs one target and is what the workflow calls"
    required: true
    manual: false
  - id: "C3"
    text: "scripts/test-release-workflow.sh gains checks for the five asset names, the signing step, and that no app bundle, installer or Velopack package includes tenmoku-server"
    required: true
    manual: false
  - id: "C4"
    text: "The finish note says the workflow edit needs a human risk approval"
    required: true
    manual: false
---

# Release workflow: build tenmoku-server for linux x86_64 and aarch64 (static musl) and macOS arm64, and publish the archives with an Ed25519-signed SHA256SUMS on every tag (remote 4c)

# Release assets for tenmoku-server (remote 4c)

D § 4 ("Release assets": names, signed sums, why a per-asset .sha256 is not enough), § 3 (targets, static musl), § 14 (the archive size is an estimate until this card builds it: record the real sizes in the lane decision file).
T2001 and T2002 also edit release.yml: this card depends on them, rebase over them. Read `.github/workflows/release.yml` and `scripts/test-release-workflow.sh` first.

Workflow edit: a change to .github/workflows/** needs a human risk approval at finish. Say so in the finish note and do not retry around it.

## Rules
Read the decision sections named above in `docs/decisions/j8wvgszah0-remote-development.md` (and the design file `docs/designs/remote-development.md` where cited) before starting; they are the spec, this card only cuts a slice of them. Do not widen --scope; if a file outside it is needed, stop and respec (notes and choices go in your lane's `docs/decisions/<lane>-*.md`, which is exempt). New tests go in new files, one fn per behaviour, small fixtures, no sleeps for timing (D § 12 spike notes). Gate tests on the affected crates only. Not here: anything the decision assigns to another phase (D § 13).

## Implementation notes

- Choices, the real archive sizes (1.6–1.9 MiB packed) and the musl `renameat2` finding are in
  `docs/decisions/abwetzhqcv-tenmoku-server-release-assets.md`.
- Reused, not copied: `crates/fastedit/examples/sign_update_feed.rs` as the signer. The
  linux-x86_64 job's build of it reaches the sums job as a one-day artifact. The key handling
  (key out of the environment first, then printf into the signer's stdin, and the public half
  compared to TENMOKU_UPDATE_PUBLIC_KEY) and the version check follow `scripts/velopack-pack.sh`
  and `scripts/package-linux.sh`. publish's existing stale-`.sig` guard and the test script's
  `job`/`step`/`publish` helpers were extended rather than duplicated. The archive layout matches
  `fe_remote_client::source::pack` and `bootstrap.sh`.
- Scope add (rev 3): `crates/fe-project/src/fs_ops.rs`, where `rename_exclusive` on Linux calls
  `renameat2` through `libc::syscall`, because Rust's bundled musl has no `renameat2` symbol.
- Copy to share (review r1 F3): the signing block in `scripts/build-tenmoku-server.sh:64` (key out
  of the environment) and `scripts/build-tenmoku-server.sh:139` (key on the signer's stdin, the
  public half checked, the .sig removed otherwise) does the job of `take_private_key` and
  `sign_feed` in `scripts/velopack-pack.sh:94` and `scripts/velopack-pack.sh:102`.
  `velopack-pack.sh` is outside this card's scope and can't be sourced without running it, so the
  follow-up is to move both functions into a sourced `scripts/lib/update-key.sh` and have both
  scripts call it. `velopack-pack.ps1` has its own PowerShell copy.
- Review r1 F1: a tag push (stable or preview) now fails in `tenmoku-server-sums` without the
  signing key, through `build-tenmoku-server.sh --sums --require-signature`. publish needs that
  job, so no tag publishes an unsigned server. A manual run still warns and stays unsigned.
- `.github/workflows/release.yml` changed, so the finish needs a human risk approval.

---
task_uid: "01m8expqhjg86z7ah39pt6ez7a"
task: "T2183"
rev: 3
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: "scripts/check-release-version.sh v0.3.1-preview.1 exits 0"
    required: true
    manual: false
  - id: "C2"
    text: "git diff of Cargo.lock changes only workspace crates' version lines from 0.3.0 to 0.3.1-preview.1"
    required: true
    manual: false
  - id: "C3"
    text: "release-notes/0.3.1-preview.1.md has New/Improved/Fixed sections in release-notes/0.3.0.md's style, lists only user-visible changes since v0.3.0, and passes scripts/check-release-version.sh --notes 0.3.1-preview.1"
    required: true
    manual: false
---

# Bump the workspace version to 0.3.1-preview.1 for the v0.3.1-preview.1 release

Bump the workspace version to 0.3.1-preview.1 for the v0.3.1-preview.1 release (user-approved 2026-11-27: cut the next release as the first preview through the new channel once T2002 lands, to test the preview pipeline end to end: T2001 preview prereleases, T2002 rolling preview release and feeds).

Changes, nothing else:
- Cargo.toml `[workspace.package]` `version = "0.3.0"` -> `"0.3.1-preview.1"`.
- Cargo.lock: refresh the workspace crates' versions with `cargo update --workspace` (no build). Only workspace crates' `version` lines change.
- release-notes/0.3.1-preview.1.md (new): start from `scripts/draft-release-notes.sh --version 0.3.1-preview.1` (it drafts from v0.3.0..HEAD), then rewrite it for users in the style of release-notes/0.3.0.md: `## New`, `## Improved`, `## Fixed`, one plain sentence per user-visible change, naming settings and commands as the app shows them. Leave out internal work (kiln-ui reuse refactors, AGENTS.md, test and flake fixes, CI). The new first-party grammars reach users through the grammar registry without an app release: list them in one `## New` bullet ("Syntax highlighting you can install for ..."), alphabetical. Include the semantic-tokens fix (highlighting no longer gives up while rust-analyzer is slow to start), the smooth spinner, the Yohen/Hakuji/Ember tag colour, Blocks (an optional plugin), and whatever else on main by then is user-visible.

One test change (rev 3, lead-approved): crates/tenmoku-server/tests/daemon_attach.rs `without_a_runtime_dir_the_daemon_listens_under_the_cache` gives its temporary home a shorter path, because the preview build ID (28 characters, repeated twice in the socket path) pushes the cache socket past the 104-byte limit and the server rightly falls back to /tmp. No product code or settings change; the shorter-ID fix is a separate follow-up card. Ask before any cargo command that builds. The lead tags the landed commit `v0.3.1-preview.1` and pushes the tag; the release workflow does the rest.

## Implementation notes

- Used `scripts/draft-release-notes.sh --version 0.3.1-preview.1` (48 commits in `v0.3.0..HEAD`),
  rewritten by hand. `cargo update --workspace --offline` changed 46 `version` lines in
  `Cargo.lock` and nothing else. No product code; the one test change reuses the `tempfile` builder
  the test support's `Host::new` uses (`crates/tenmoku-server/tests/common/mod.rs`).
- **Blocks:** the app can't download a registry plugin yet (T2076 and T2080 are blocked;
  `docs/release/plugin-registry.md`, "What apps do with it today"), and code plugins need
  `extensions.plugins_preview`. The bullet says how to install it by hand rather than implying
  the Extensions list offers it. `tenmoku.blocks` 0.1.0 is in the live registry's releases.
- **Added beyond the spec's list:** Preview releases (`update.channel` shipped in 0.3.0 but no
  preview existed and its notes didn't mention it), Mermaid diagrams (T2181), `panels.reveal`
  (T2009), `.m` detection (T2136), Home and End in three menus (a side effect of T2155), and
  non-executable `gradlew`/`mvnw` (T2117).
- **Left out:** internal work and test fixes; the Jinja long `{% filter %}` chain fix (T744: five
  or more filters, and reaches only new grammar installs); locale-switch refresh (T2115: only
  en-US ships); What's New preview versions (T2184: only matters on a preview build); the
  empty-state type size change (T2153).
- **Preview build IDs and the cache socket (rev 3):** the build ID is in
  `<home>/.cache/tenmoku-server/<build>/run/<build>.sock` twice. With
  `0.3.1-preview.1+<12 hex>` (28 characters) a home over 14 bytes falls back to
  `/tmp/tenmoku-server-<uid>/` (over 34 bytes with `0.3.0`), so
  `without_a_runtime_dir_the_daemon_listens_under_the_cache` now uses a 9-byte temporary home.
  In a dirty tree the ID also ends in `-dirty` (34 characters), which leaves room for no
  temporary home, so that test still fails on uncommitted changes while the version is a
  preview; the gate's clean snapshot passes it. The lead is filing the build-ID follow-up.


---
task_uid: "01m8f07dchmyy6zy7r0fvj9zh0"
task: "T2184"
rev: 3
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "`a_preview_after_a_stable_or_an_earlier_preview_shows_whats_new` (crates/fe-config/tests/whats_new_preview.rs) fails at base and passes: decide gives Show for 0.3.1-preview.1 after seen 0.3.0 and for preview.2 after seen preview.1, Seen for preview.1 after seen preview.1"
    required: true
    manual: false
  - id: "C2"
    text: "`the_tab_shows_a_running_preview_and_not_the_next_stable_draft` (crates/fe-workbench/src/tests/whats_new_preview.rs): notes_markdown with 0.3.0, 0.3.1-preview.1 and 0.3.1 and running 0.3.1-preview.1 gives the preview's heading, then 0.3.0's, and no 0.3.1 heading"
    required: true
    manual: false
  - id: "C3"
    text: "fe-workbench builds with a release-notes/X.Y.Z-preview.N.md file present (shown in the lane, noted in Implementation notes) and still fails on a file named otherwise; the embedded-notes rules test in crates/fe-workbench/src/tests/whats_new.rs passes"
    required: true
    manual: false
---

# Preview versions in release notes and What's New: build.rs accepts X.Y.Z-preview.N.md, and the What's New tab and its show-after-update decision compare preview versions

## Problem

Preview versions (`X.Y.Z-preview.N`, channels work T1996-T2002) can't be released: T2183 bumps the workspace to `0.3.1-preview.1` and adds `release-notes/0.3.1-preview.1.md`, and three places still read only `X.Y.Z` (found by T2003's worker, 2026-11-27):

1. `crates/fe-workbench/build.rs` `parse_version` panics on any notes file not named `X.Y.Z.md`, so the preview's notes file fails every fe-workbench build.
2. `crates/fe-workbench/src/whats_new.rs` `notes_markdown` parses `running` and each notes version with `fe_config::whats_new::parse_version`: a preview running version gives `limit = None` (every file shown, drafts too), and the preview's own notes are dropped.
3. `crates/fe-config/src/whats_new.rs` `decide` parses `running` with `parse_version`: a preview always gives `WhatsNew::Seen`, so What's New never opens after updating to a preview. The marker's `seen` value is parsed the same way.

fe-config already has the preview-aware `ReleaseVersion` and `parse_release_version` (`crates/fe-config/src/whats_new.rs` about line 80; a preview sorts below its own `X.Y.Z` and above the stable before it). Use it everywhere above.

## Change

- `build.rs`: accept `X.Y.Z.md` and `X.Y.Z-preview.N.md` (same rules as `parse_release_version`; copy it, since a build script can't depend on fe-config, as the existing copy of `parse_version` notes), sort newest first by that order; still fail on any other name.
- `whats_new.rs` (workbench): `notes_markdown` compares with `parse_release_version` for the limit and each file; the running preview's notes show under its heading; a later stable's draft (`0.3.1.md` while running `0.3.1-preview.1`) is left out.
- `whats_new.rs` (fe-config): `decide` parses `running` and `seen` with `parse_release_version`.
- `crates/fe-workbench/src/tests/whats_new.rs`: the embedded-notes rules test (about line 704) parses with `parse_release_version` and checks the name round-trips for previews too. No other change there.
- `docs/release/channels.md` (added by T2003): remove the note above "Cutting a preview" and the first "Known gaps" bullet, which describe this build failure. Leave the second "Known gaps" bullet (the GitHub update check ignoring the channel): it is T1994's.
- Leave `crates/fe-workbench/src/update_check.rs` alone: its prerelease compare is T1994's (lent to neon).

## Tests

- Regression, new file `crates/fe-config/tests/whats_new_preview.rs` (base API only: `fe_config::whats_new::{decide, MARKER_FILE, WhatsNew}`, writing `seen = "0.3.0"` into a temp state dir's marker file): `decide(dir, "0.3.1-preview.1")` is `WhatsNew::Show`; with `seen = "0.3.1-preview.1"`, `decide(dir, "0.3.1-preview.2")` is Show and `decide(dir, "0.3.1-preview.1")` is Seen. Use `assert_eq!` so a failure is an assertion.
- New unit test file `crates/fe-workbench/src/tests/whats_new_preview.rs` declared from `crates/fe-workbench/src/tests/mod.rs`: `notes_markdown` with notes 0.3.0, 0.3.1-preview.1 and 0.3.1 and running `0.3.1-preview.1` shows the preview's then 0.3.0's headings in that order and not 0.3.1's.
- Prove build.rs by hand in the lane: add a scratch `release-notes/0.3.1-preview.1.md` that passes the rules, build fe-workbench, remove it before committing (T2183 adds the real one). Note the result in Implementation notes.

## Rules

Do not widen --scope; if a file outside it is needed, stop and respec. Ask before any cargo build and before rgt finish.

## Implementation notes

- Reused `fe_config::whats_new::parse_release_version` and `ReleaseVersion`'s order in
  `decide`, `notes_markdown` and the embedded-notes rules test.
- Copy, as the spec asks: `crates/fe-workbench/build.rs` repeats `parse_release_version` and
  `parse_version` (originals in `crates/fe-config/src/whats_new.rs`), returning a sortable
  tuple in place of `ReleaseVersion`. Sharing them would need a build dependency.
  `crates/fe-workbench/src/update_check.rs` has its own `parse_version`, left to T1994.
- build.rs by hand (2026-11-27, lane): with a scratch `release-notes/0.3.1-preview.1.md`
  (`## Fixed`, one bullet), fe-config and fe-workbench built, the generated `RELEASE_NOTES`
  listed `0.3.1-preview.1` first, above `0.3.0`, and the 20 targeted What's New and release
  version tests passed, `the_shipped_notes_are_well_formed` among them. Adding a scratch
  `release-notes/0.3.1-rc.1.md` then failed `cargo build -p fe-workbench` in the build
  script: "release-notes/0.3.1-rc.1.md: a release's notes are named X.Y.Z.md or
  X.Y.Z-preview.N.md, ...". Both scratch files were removed.
- `docs/decisions/d7t5rfkttd-whats-new.md` still describes the `X.Y.Z.md`-only rule, as
  the record of that card's decision; it was not edited.

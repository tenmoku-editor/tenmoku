---
task_uid: "01m7th35g8xt46hvxkg0nn9xyv"
task: "T2003"
rev: 1
type: "docs"
complexity: "xs"
criteria:
  - id: "C1"
    text: "docs/release/channels.md describes: versions (`0.3.0-preview.N`, `0.3.0`), when to cut which, the commands to cut a preview (Cargo.toml bump, release-notes file, tag), where the preview feeds live, and the user-facing switch (Settings \u003e Update \u003e Channel)."
    required: true
    manual: false
  - id: "C2"
    text: "It states the return path plainly: choosing Stable does not downgrade; the editor stays on the preview until the next stable release newer than it (the preview feed also carries every stable, so a preview user receives that stable as an ordinary update); a user who wants stable immediately installs the latest stable download over it, which platforms.md and the release's Downloads line show."
    required: true
    manual: false
  - id: "C3"
    text: "platforms.md's update section and macos-signing.md's Releasing section link to it and mention the two feed URLs."
    required: true
    manual: false
---

# Document the release channels: how to cut a preview, how a user switches, and back to stable (channels 11)

# Channels documentation

Docs only. Written after the workflow cards so it describes what shipped. Read docs/release/platforms.md and docs/release/macos-signing.md.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

## Implementation notes

- Docs only; no code components added or reused. channels.md links to macos-signing.md,
  "Releasing", for the notes rules and secrets rather than repeating them.
- **Follow-up needed before any preview can be cut (blocks T2183):**
  crates/fe-workbench/build.rs:25 panics on a `release-notes/` file not named `X.Y.Z.md`
  (reproduced by compiling build.rs alone and running it on a tree holding
  `0.3.1-preview.1.md`), and crates/fe-workbench/src/tests/whats_new.rs:704 unwraps the same
  three-part parse. scripts/check-release-version.sh requires that file for a preview tag.
  T1995's scope lists neither build.rs nor that test.
- The three-part parser is copied at crates/fe-workbench/build.rs:52,
  crates/fe-config/src/whats_new.rs:68 and crates/fe-workbench/src/update_check.rs:302;
  fe-config's `parse_release_version` already accepts previews.
- T1994 (not landed): on a preview build with no updater hook, Check for Updates fails with
  "can't be compared" (update_check.rs:266).
- channels.md states both gaps under "Known gaps" and above the preview steps; whoever fixes
  them should remove that text.
- The Windows downgrade prompt is from Velopack's source at the pinned commit 8d2844f
  (`show_overwrite_repair_dialog`); no installer was run.

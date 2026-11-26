---
task_uid: "01m7whzds2cyem428apf8n15p4"
task: "T390"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "On the stable channel the GitHub fallback check behaves as today (releases/latest, no prereleases, tag `vX.Y.Z` only) and compares with `ReleaseVersion`, so a running `0.3.0-preview.2` is older than a latest `v0.3.0` and the toast offers it."
    required: true
    manual: false
  - id: "C2"
    text: "On the preview channel the fallback lists releases (`/repos/tenmoku-editor/tenmoku/releases?per_page=10`), takes the newest non-draft whose tag is `vX.Y.Z` or `vX.Y.Z-preview.N` by `ReleaseVersion` order, and offers it only if newer than the running version; the page must still start with RELEASE_PAGES. The toast names the channel (`Tenmoku 0.3.0-preview.4 (preview) is available`) through a new Fluent string."
    required: true
    manual: false
  - id: "C3"
    text: "Tests in crates/fe-workbench/tests/update_check_channel.rs, with the fake UpdateSource (no network): `stable_channel_ignores_a_newer_preview`, `preview_channel_offers_a_newer_preview`, `preview_channel_offers_the_stable_that_follows_the_running_preview`, `preview_channel_never_offers_a_downgrade`, `draft_and_malformed_tags_are_skipped_on_both_channels`."
    required: true
    manual: false
  - id: "C4"
    text: "The updater hook (set_updater_hook) path is unchanged. `cargo test -p fe-workbench update_check` passes."
    required: true
    manual: false
---

# Check for Updates fallback follows the channel and compares prerelease versions (channels 2) (Mac T1994)

# Check for Updates fallback: channel-aware

The fallback (Linux and Windows builds with no signed feed, and any build with no hook) asks GitHub's API. Make it honour `update.channel` (CH1) and compare with `ReleaseVersion`. Files read first: crates/fe-workbench/src/update_check.rs (LATEST_RELEASE, the prerelease filter near line 290), update_check_tests.rs, crates/fe-config/src/whats_new.rs after CH1. Read the setting through the same Settings global the Check for Updates command already reaches; if it cannot be read where the check runs, pass it in from the command wrapper in update_check.rs and say so.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

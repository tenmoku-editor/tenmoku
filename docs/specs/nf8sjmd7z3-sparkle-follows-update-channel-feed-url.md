---
task_uid: "01m7th31kq9pkyv5nf8sjmd7z3"
task: "T1999"
rev: 1
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "The Info.plist gains `TenmokuPreviewFeedURL` (written by bundle-macos.sh from TENMOKU_PREVIEW_FEED_URL, default https://github.com/tenmoku-editor/tenmoku/releases/download/preview/appcast-preview.xml; https only) beside SUFeedURL."
    required: true
    manual: false
  - id: "C2"
    text: "A SPUUpdaterDelegate (objc2 class in sparkle.rs's macOS module, passed as updaterDelegate where it is nil today) answers `feedURLStringForUpdater:` with the preview feed when the setting is preview and nil (stable SUFeedURL) otherwise; the channel is read at call time so a settings change applies to the next check without restart, and a settings reload re-reads it."
    required: true
    manual: false
  - id: "C3"
    text: "A pure, all-OS function `feed_for(channel, stable, preview) -\u003e FeedChoice` carries the decision; tests in crates/fastedit/src/sparkle_tests.rs: `stable_uses_the_plist_feed`, `preview_uses_the_preview_feed`, `a_missing_or_non_https_preview_feed_falls_back_to_stable`."
    required: true
    manual: false
  - id: "C4"
    text: "scripts/test-sparkle-bundle.sh asserts the plist key is present and https; the T1373 smoke recipe (throwaway bundle id and HOME) shows a preview check requesting the preview URL (the finish note records the logged URL)."
    required: true
    manual: false
  - id: "C5"
    text: "Switching the setting never offers a downgrade: Sparkle already refuses older versions; a test of the version comparison for preview -\u003e stable lives in CH4."
    required: true
    manual: false
---

# Sparkle follows update.channel: feed URL from the setting (channels 7)

# Sparkle channel from the setting (macOS)

Mac-only: objc2 delegate and the framework. Files read first: crates/fastedit/src/sparkle.rs (Policy, platform::start passing nil delegates at ~line 250), sparkle_tests.rs, crates/fastedit/macos/Info.plist (SU* keys), scripts/bundle-macos.sh, memory notes on T1373 (failed updater start keeps the hook; project file can set automatic_checks: here the channel is user-only, via CH1). Depends on CH1 for the setting, not on CH5.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

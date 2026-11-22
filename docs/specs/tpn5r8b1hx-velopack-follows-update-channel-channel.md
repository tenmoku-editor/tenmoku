---
task_uid: "01m7th32qt766nj1tpn5r8b1hx"
task: "T2000"
rev: 1
type: "feature"
complexity: "s"
criteria:
  - id: "C1"
    text: "`release_channel(os, arch)` becomes `release_channel(os, arch, channel)` returning `windows-x64-preview` etc. for preview; the preview feed is a second baked URL `TENMOKU_UPDATE_FEED_PREVIEW` (option_env, https or absolute path as the stable one, same public key) and a build without it checks the stable feed whatever the setting says, with a logged reason."
    required: true
    manual: false
  - id: "C2"
    text: "The feed signature check, package allow-listing and downgrade behaviour are unchanged: velopack's AllowVersionDowngrade stays off. Tests in velopack_update_tests.rs: `preview_channel_names_per_os_and_arch`, `preview_without_a_preview_feed_checks_stable`, `preview_feed_with_a_bad_signature_is_refused`, `preview_never_installs_an_older_version`."
    required: true
    manual: false
  - id: "C3"
    text: "The channel is read when a check starts, so a settings change applies to the next check."
    required: true
    manual: false
---

# Velopack follows update.channel: channel name and preview feed (channels 8)

# Velopack channel from the setting (Windows and Linux)

Files read first: crates/fastedit/src/velopack_update.rs (FEED at line 40, release_channel at ~152, Policy, manager()) and its tests. The default stable feed is `https://github.com/<repo>/releases/latest/download`; CH10 builds the preview feed at `.../releases/download/preview`.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

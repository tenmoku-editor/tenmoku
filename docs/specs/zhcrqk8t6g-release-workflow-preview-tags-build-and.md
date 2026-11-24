---
task_uid: "01m7th33tkfhvmstzhcrqk8t6g"
task: "T2001"
rev: 1
type: "chore"
complexity: "m"
criteria:
  - id: "C1"
    text: "A tag matching `v*-preview.*` runs the same four platform jobs and the version job checks it with CH4's script; the publish job creates the release with `--prerelease --latest=false` and never touches `releases/latest`; a stable tag is unchanged (`--latest`)."
    required: true
    manual: false
  - id: "C2"
    text: "The macOS job passes the preview version to bundle-macos.sh and writes `appcast-preview.xml` (CH5) for a preview and, for a stable tag, both `appcast.xml` and `appcast-preview.xml`; the Windows and Linux jobs pass `--channel-set both` for a stable and the default for a preview (CH6); every build bakes TENMOKU_UPDATE_FEED_PREVIEW (default `https://github.com/{repo}/releases/download/preview`)."
    required: true
    manual: false
  - id: "C3"
    text: "scripts/test-release-workflow.sh gains cases (it already parses the workflow): prerelease flags for a preview tag, `--latest` only for stable, the preview appcast written in both flows, and no `appcast.xml` for a preview."
    required: true
    manual: false
  - id: "C4"
    text: "The existing stale-feed handling (a re-run of a tag removes feed files whose .sig would be stale) covers the preview files too."
    required: true
    manual: false
---

# Release workflow: preview tags build and publish as GitHub prereleases (channels 9)

# Release workflow: preview tags

Files read first: .github/workflows/release.yml (version job, the macOS Sparkle step ~line 120, the publish step ~150-190), scripts/test-release-workflow.sh (the shape of the checks at line ~242). This card makes the tagged release correct; CH10 adds the rolling `preview` feed release.


Workflow edit: a change to .github/workflows/** needs a human risk approval at finish. Say so in the finish note and do not retry around it.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

## Implementation notes

- C2's "the default for a preview": velopack-pack.sh's and .ps1's default set is `stable`, which refuses a preview version (T1998's notes), so the workflow passes `--channel-set both` / `-ChannelSet both` on every run. On a preview version `both` packs `<os>-<arch>-preview` only; on a stable one, both channels. That is what CH6's help text says the release workflow passes.
- TENMOKU_UPDATE_FEED_PREVIEW (`vars.TENMOKU_UPDATE_FEED_PREVIEW`, default `https://github.com/{repo}/releases/download/preview`) is baked into the Windows and Linux builds, the ones that read it (velopack_update.rs). The macOS build updates through Sparkle: its preview feed is the bundle step's TENMOKU_PREVIEW_FEED_URL (`vars.TENMOKU_PREVIEW_FEED_URL`, default `https://github.com/{repo}/releases/download/preview/appcast-preview.xml`), repo-derived like TENMOKU_FEED_URL rather than bundle-macos.sh's hard-coded default.
- The tag trigger (`v*`) and the version job (check-release-version.sh, which already takes `vX.Y.Z-preview.N` at Cargo.toml's version) needed no change; the test runs the check on a preview fixture and refuses any job or step conditional on the tag's name.
- publish: a preview is created with `--prerelease --latest=false` and a re-run's `gh release edit` passes the same flags, so a re-run keeps it off latest. A new stable gets `--latest`; a stable re-run passes no flag, so re-running an old stable tag never takes latest from a newer release.
- Stale feeds (C4): the existing guard refuses (it removes nothing) a re-run that made no file whose old copy and .sig are on the release. It now loops over `appcast.xml` and `appcast-preview.xml`; the Velopack loop already matched `releases.*.json.sig`, preview channels included, and is now tested with one. A preview's dist holding `appcast.xml` or a `releases.*-stable.json` is refused before any gh call.
- This changes .github/workflows/**, which needs a human risk approval at finish. Nothing here ran the workflow on GitHub; the first preview tag is its real run (CH10 adds the rolling `preview` release the baked feeds point at).
- docs/release/macos-signing.md's Releasing section does not yet describe preview tags (out of scope).

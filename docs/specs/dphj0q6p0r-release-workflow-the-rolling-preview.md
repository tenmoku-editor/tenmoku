---
task_uid: "01m7th34r97659qhdphj0q6p0r"
task: "T2002"
rev: 1
type: "chore"
complexity: "m"
criteria:
  - id: "C1"
    text: "After a tagged release publishes (preview or stable), a new step runs scripts/publish-preview-feed.sh, which updates the single prerelease with tag `preview` (created on first run, never marked latest): it uploads `appcast-preview.xml`, `releases.\u003cos\u003e-\u003carch\u003e-preview.json`, their `.sig` files and the `.nupkg` packages the feeds list, and removes the previous build's packages and any feed or `.sig` it replaces, so a stale `.sig` never sits beside a new feed."
    required: true
    manual: false
  - id: "C2"
    text: "The script is idempotent per tag (a re-run of the same tag gives the same assets) and refuses to publish a build older than the one already in the rolling release (never moves the channel backwards): tests with a fake `gh` in scripts/test-publish-preview-feed.sh: first create, preview after preview, stable after preview, re-run, and an older build refused."
    required: true
    manual: false
  - id: "C3"
    text: "The Sparkle enclosure URLs in appcast-preview.xml point at the tagged release's zip, not at the rolling release; Velopack packages are in the rolling release because the feed resolves them beside itself. State in the finish note the rolling release's size at steady state."
    required: true
    manual: false
---

# Release workflow: the rolling preview release holds the preview feeds and newest packages (channels 10)

# The rolling `preview` release

GitHub's releases/latest/download never lists prereleases, so the preview channel's feeds live in one rolling prerelease tagged `preview`. Files read first: release.yml publish step (the --clobber/stale-sig handling), memory note T1374 (--clobber leaves a stale appcast), scripts/publish-grammar-registry.sh (a model for a script that publishes assets with a fake gh test).


Workflow edit: a change to .github/workflows/** needs a human risk approval at finish. Say so in the finish note and do not retry around it.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

## Implementation notes

- The step is in a job of its own, `preview-feed` (needs `publish`, tag pushes only,
  `contents: write`), with `concurrency: {group: preview-feed, cancel-in-progress: false,
  queue: max}`, so two tags' runs never interleave the check and the uploads, and no pending run
  is dropped: the default (`queue: single`) cancels a pending run when another queues, which could
  drop the newest build's run behind an older one's (review F1). With every run kept, the order
  they run in doesn't matter: the newest publishes and an older one run after it is refused.
  actionlint 1.7.12 (the latest) doesn't know `queue` yet, so scripts/test-release-workflow.sh
  ignores that one report.
- The rolling release's build is recorded as a watermark line in its notes,
  `Preview build: version=<v> tag=<tag>`, written before any upload (as the grammar registry's
  index release does), so a failed upload still records the newest build that started. Order:
  X.Y.Z, then stable after every X.Y.Z-preview.N, then N. The same version is a re-run.
- Upload order: packages, then per feed (delete old .sig, upload feed, upload .sig), then
  appcast-preview.xml, then delete the previous build's packages. A feed is briefly unsigned
  between the delete and its .sig upload, which the app refuses (a transient failed check), but
  never paired with another feed's .sig.
- Refused, publishing nothing: an older build; a draft; assets with no watermark; an enclosure
  other than exactly `https://github.com/<repo>/releases/download/<tag>/Tenmoku-<v>-macos-arm64.zip`
  (review F2: the rolling release, another tag, another repository or another host; a
  `DOWNLOAD_URL_PREFIX` variable pointing elsewhere, unset on the repository today, would make
  the step refuse); a feed of the rolling release
  this build doesn't replace (its packages would go); a signed feed or an appcast replaced by
  none.
- Steady state: one appcast-preview.xml (~6 KB), three feeds (~270 B each), three .sig files
  once the update key is set, and three full .nupkg: at v0.3.0's sizes 53,316,438 (linux-x86_64)
  + 43,616,027 (windows-arm64) + 48,142,981 (windows-x64) = 145,075,446 bytes, about 145 MB
  (138 MiB) in 10 assets (7 unsigned). The zips and installers stay in the tagged releases.
- scripts/test-release-workflow.sh: the job list and the `contents: write` check now include
  preview-feed, and its step-body extractor stops at the next job (the publish step is no
  longer the file's last).

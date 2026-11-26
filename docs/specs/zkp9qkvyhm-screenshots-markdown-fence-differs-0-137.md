---
task_uid: "01m7qwzr1ke7p8ztzkp9qkvyhm"
task: "T1958"
rev: 2
type: "chore"
complexity: ""
criteria:
  - id: "C1"
    text: "markdown_fence passes 5 consecutive screenshots runs on Mac with no tolerance change, or the cause is documented"
    required: true
    manual: false
---

# Screenshots markdown_fence differs 0.137% (2108 px) on every Mac run since at least 023b28b42: bold/italic heading, emphasis and strong glyphs drifted from the golden

Lead update 2026-11-18: not a flake. markdown_fence fails with exactly 2108 of 1536000 pixels (0.137%) on every run on this Mac: stack 220's gate, stack 221's gate and its rerun on a5b3e7fe7, and the base render on 023b28b42 is byte-identical to the a5b3e7fe7 render. Treat it as deterministic drift: find which change moved the bold/italic heading, emphasis and strong glyphs (bisect the golden's last refresh against main), then either fix the regression or refresh the golden with the cause in the commit.

## Implementation notes

- **Cause.** T1924 (`182daac17`, Markdown and markup font styles) draws
  headings and `**strong**` bold and `*emphasis*` italic. It left
  `markdown_fence`'s golden, last written by T1600 (`df5e32d98`), as it was:
  its notes say no existing golden shows a Markdown file, and its screenshot
  criterion ran fe-workbench's example only. An intended change with a stale
  golden, not a regression.
- **Bisect.** `cargo run -p fe-gpui --example screenshots` on exported trees:
  `markdown_fence` matches at `64e0b5b75` (T1924's parent) and differs by 2108
  of 1536000 pixels at `182daac17`, as at the base `9ffb2efa5`; the two
  failing renders are byte-identical. (Both exported trees also differ in
  `which_key_icons`, since refreshed by T1957.)
- **Fix.** Only `golden/markdown_fence.png` is regenerated: it is the base's
  render, byte for byte. The diff against the old golden covers just `Notes`,
  `emphasis` and `strong`. `UPDATE_SCREENSHOTS=1` also rewrote other goldens,
  which matched already; those were restored.
- **C1.** Five consecutive runs on the Mac, tolerance unchanged: all 17 scenes
  `ok` each time.

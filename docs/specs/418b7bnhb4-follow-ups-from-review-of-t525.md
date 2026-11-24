---
task_uid: "01m842k6f3tyhynf418b7bnhb4"
task: "T745"
rev: 1
type: "chore"
complexity: "s"
criteria: []
---

# Follow-ups from review of T525

Findings of the review of T525 that don't block the merge (below merge.block_severity):

- F2 medium crates/fe-vim/l10n/en-US/vim.ftl:424: The new replay-cap message interpolates a count without a plural selector, contrary to the spec's explicit requirement. The existing vim-macro-replay-capped message already expresses the same text with a selector.
  fix: Reuse the existing replay-cap message/helper, or add a selector on $cap while preserving the current en-US output.
  instances: crates/fe-vim/l10n/en-US/vim.ftl:424

---
task_uid: "01m84nk8q8xgavz80vsxkcqekh"
task: "T746"
rev: 1
type: "chore"
complexity: "s"
criteria: []
---

# Follow-ups from review of T542

Findings of the review of T542 that don't block the merge (below merge.block_severity):

- F1 medium extensions/grammars/liquid/test/sample.liquid:48: The sample omits the style_content injection produced by {% style %}; exercising {% stylesheet %} covers a different query pattern. It also contains no strings with escapes. Both are explicitly required sample coverage, and the golden therefore cannot detect regressions in either case.
  fix: Add a {% style %} block and a string containing an escape sequence, keep the sample within 40–120 lines, then regenerate and inspect the golden and rerun Liquid parity.
  instances: extensions/grammars/liquid/test/sample.liquid:45, extensions/grammars/liquid/test/sample.liquid:48

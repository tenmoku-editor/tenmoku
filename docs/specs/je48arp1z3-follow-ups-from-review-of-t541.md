---
task_uid: "01m83qcbnfeb90k3je48arp1z3"
task: "T744"
rev: 1
type: "chore"
complexity: "m"
criteria: []
---

# Follow-ups from review of T541

Findings of the review of T541 that don't block the merge (below merge.block_severity):

- F1 medium extensions/grammars/jinja/highlights.scm:225: The original chained-filter defect remains beyond four filters. In `{% filter trim | striptags | lower | upper | escape %}text{% endfilter %}`, trim sits beneath five binary_expression nodes and matches none of these finite-depth patterns. Subsequent filters are coloured by the pipe pattern. The new three-filter sample does not exercise this boundary.
  fix: Replace the finite-depth workaround with highlighting that covers the first bare filter regardless of chain length, and add a regression sample with at least five filters before regenerating its golden.
  instances: extensions/grammars/jinja/highlights.scm:198, extensions/grammars/jinja/highlights.scm:208, extensions/grammars/jinja/highlights.scm:216, extensions/grammars/jinja/highlights.scm:225

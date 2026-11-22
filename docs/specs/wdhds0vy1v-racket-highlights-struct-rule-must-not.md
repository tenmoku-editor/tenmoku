---
task_uid: "01m80txs05df5zkswdhds0vy1v"
task: "T436"
rev: 1
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "In extensions/grammars/racket/highlights.scm the struct rule is placed before the quote and datum-comment overrides, so a quoted ('(struct ...)) or datum-commented (#;(struct ...), #;(define-struct ...)) struct form keeps its quote and comment colouring instead of struct colours"
    required: true
    manual: false
  - id: "C2"
    text: "The racket sample and its regenerated golden gain quoted and datum-commented struct and define-struct cases, and the racket grammar's highlight tests pass; only extensions/grammars/racket/** changes"
    required: true
    manual: false
---

# Racket highlights: struct rule must not recolour quoted or datum-commented forms (review follow-up of T1480 F1) (Mac T2093)

Findings of the review of T1480 that don't block the merge (below merge.block_severity):

- F1 medium extensions/grammars/racket/highlights.scm:114: The added struct rule follows the quote and datum-comment overrides despite later-pattern precedence. In #;(struct point (x y)) and #;(define-struct point (x y)), it recolours the commented keyword and name as keyword/type; quoted equivalents likewise lose their constant colouring. The sample does not cover these overlaps.
  fix: Move the struct rule before the quote and datum-comment overrides, and add quoted and datum-commented struct/define-struct examples to the sample and regenerated golden.
  instances: extensions/grammars/racket/highlights.scm:114


(Argon note: the Mac card carries no criteria; the two below are taken from the finding's fix text.)

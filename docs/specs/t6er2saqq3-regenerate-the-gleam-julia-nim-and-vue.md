---
task_uid: "01m7zkddhpny1brgt6er2saqq3"
task: "T741"
rev: 2
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: "wasm_parity::every_extension_highlights_its_sample passes with the four goldens rewritten by UPDATE_GOLDEN=1, and the diff shows only the intended class changes: markup.italic/markup.strong/markup.link.label and the markup.list list markers from T1924's markdown queries, and function.definition from T674"
    required: true
    manual: false
---

# Regenerate the gleam, julia, nim and vue extension goldens for the markdown-inline and function.definition capture changes (Grammars workflow red since 11-17) (Mac T2085)

## Implementation notes

Goldens rewritten with UPDATE_GOLDEN=1. Class changes: markup.italic/markup.strong/markup.link.label (T1924), function.definition (T674), and gleam's " - " list markers punctuation.special -> markup.list (also T1924: list_marker_minus @markup.list in markdown.scm; accepted by the helium lead).

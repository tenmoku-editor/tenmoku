; From tree-sitter-dot at 80327abbba6f47530edeb0df9f11bd5d5c93c14d,
; https://github.com/rydesun/tree-sitter-dot, path queries/highlights.scm,
; license MIT. Changes: `@namespace` is named `@module`, and drops
; `(ERROR) @error`, which the editor has no colour for. Earlier patterns win,
; as upstream orders them.

[
  "strict"
  "graph"
  "digraph"
  "subgraph"
  "node"
  "edge"
] @keyword
(string_literal) @string
(number_literal) @number

[
  (edgeop)
  (operator)
] @operator

[
  ","
  ";"
] @punctuation.delimiter

[
  "{"
  "}"
  "["
  "]"
  "<"
  ">"
] @punctuation.bracket

(subgraph
  id: (id
    (identifier) @module)
)

(attribute
  name: (id
    (identifier) @type)
)

(attribute
  value: (id
    (identifier) @constant)
)

[
(comment)
(preproc)
] @comment

(identifier) @variable

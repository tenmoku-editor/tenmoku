; From tree-sitter-asm at 839741fef4dab5128952334624905c82b40c7133,
; https://github.com/RubixDev/tree-sitter-asm, path queries/asm/highlights.scm,
; licence MIT (the grammar's own, see LICENSE). nvim-treesitter's copy is the
; same query, so this is the grammar's own file and no LICENSE-APACHE is needed.
; Changes: Neovim's `@spell` is dropped (fe-syntax has no such highlight), and
; `@number.float` is written `@number`, the name in fe-syntax's vocabulary.

; General
(label
  [(ident) (word)] @label)

(reg) @variable.builtin

(meta
  kind: (_) @function.builtin)

(instruction
  kind: (_) @function.builtin)

(const
  name: (word) @constant)

; Comments
[
  (line_comment)
  (block_comment)
] @comment

; Literals
(int) @number

(float) @number

(string) @string

; Keywords
[
  "byte"
  "word"
  "dword"
  "qword"
  "ptr"
  "rel"
  "label"
  "const"
] @keyword

; Operators & Punctuation
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "|"
  "^"
  "&"
] @operator

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  ","
  ":"
] @punctuation.delimiter

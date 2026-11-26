; From tree-sitter-bibtex at 8d04ed27b3bc7929f14b7df9236797dab9f3fa66,
; https://github.com/latex-lsp/tree-sitter-bibtex, path queries/highlights.scm,
; licence MIT (the grammar's own, see LICENSE). Every capture is already in
; fe-syntax's vocabulary and the query uses no predicates. One change: upstream
; captures the whole (command) node, whose extent covers a braced argument, so
; the gaps between the words inside `\emph{a b c}` are coloured as the command
; while the words themselves are strings. Capturing the command's name alone
; leaves the argument to the patterns for its own nodes.

[
  (string_type)
  (preamble_type)
  (entry_type)
] @keyword

[
  (junk)
  (comment)
] @comment

[
  "="
  "#"
] @operator

(command
  name: (command_name) @function.builtin)

(number) @number

(field
  name: (identifier) @variable.builtin)

(token
  (identifier) @variable.parameter)

[
  (brace_word)
  (quote_word)
] @string

[
  (key_brace)
  (key_paren)
] @attribute

(string
  name: (identifier) @constant)

[
  "{"
  "}"
  "("
  ")"
] @punctuation.bracket

"," @punctuation.delimiter

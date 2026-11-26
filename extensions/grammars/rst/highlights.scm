;; From nvim-treesitter's runtime/queries/rst/highlights.scm at
;; 52773f0d13a0c11d0c3ef33c14ab6ecc1c17148b (https://github.com/nvim-treesitter/nvim-treesitter),
;; Apache-2.0 licence (see LICENSE-APACHE). The grammar, stsewd/tree-sitter-rst,
;; has no queries of its own.
;;
;; Changes: `@keyword.import` is `@keyword` and `@markup.raw.block` is
;; `@markup.raw`. `@spell` and `@nospell` are dropped, and with them the two
;; patterns that captured only `@spell` (paragraphs, and the bodies of
;; directives other than code).
;; `@none` is dropped with its patterns: the `doctest_block` reset (the
;; injected Python colours it) and the `:math:` role resets (the injected
;; LaTeX colours them, over the role's `@markup.raw`). A block quote is
;; `@markup.quote` only after a sibling: a directive's body injected as
;; reStructuredText (injections.scm) is one block quote, which isn't a quote.

; Marks
[
  ".."
  "|"
  "--"
  "__"
  ":"
  "::"
  (transition)
] @punctuation.special

"bullet" @markup.list

; Directives
(directive
  name: (type) @function)

(directive
  body: (body
    (arguments) @variable.parameter))

((directive
  name: (type) @keyword)
  (#eq? @keyword "include"))

(directive
  name: (type) @function.builtin
  (#any-of? @function.builtin
    ; https://docutils.sourceforge.io/docs/ref/rst/directives.html
    "attention" "caution" "danger" "error" "hint" "important" "note" "tip" "warning" "admonition"
    "image" "figure" "topic" "sidebar" "line-block" "parsed-literal" "code" "math" "rubric"
    "epigraph" "highlights" "pull-quote" "compound" "container" "table" "csv-table" "list-table"
    "contents" "sectnum" "section-numbering" "header" "footer" "target-notes" "meta" "replace"
    "unicode" "date" "raw" "class" "role" "default-role" "title" "restructuredtext-test-directive"))

; Blocks
[
  (literal_block)
  (line_block)
] @markup.raw

(_
  (_)
  .
  (block_quote
    (attribution)? @markup.italic) @markup.quote)

(substitution_definition
  name: (substitution) @constant)

(footnote
  name: (label) @constant)

(citation
  name: (label) @constant)

(target
  name: (name)? @markup.link.label
  link: (_)? @markup.link)

; Lists
; Definition lists
(list_item
  (term) @markup.strong
  (classifier)? @markup.italic)

; Field lists
(field
  (field_name) @constant)

; Inline markup
(emphasis) @markup.italic

(strong) @markup.strong

(standalone_hyperlink) @string.special.url

(role) @function

((role) @function.builtin
  (#any-of? @function.builtin
    ; https://docutils.sourceforge.io/docs/ref/rst/roles.html
    ":emphasis:" ":literal:" ":code:" ":math:" ":pep-reference:" ":PEP:" ":rfc-reference:" ":RFC:"
    ":strong:" ":subscript:" ":sub:" ":superscript:" ":sup:" ":title-reference:" ":title:" ":t:"
    ":raw:"))

[
  "interpreted_text"
  (literal)
] @markup.raw

; Prefix role
((interpreted_text
  (role) @_role
  "interpreted_text" @markup.italic)
  (#eq? @_role ":emphasis:"))

((interpreted_text
  (role) @_role
  "interpreted_text" @markup.strong)
  (#eq? @_role ":strong:"))

; Suffix role
((interpreted_text
  "interpreted_text" @markup.italic
  (role) @_role)
  (#eq? @_role ":emphasis:"))

((interpreted_text
  "interpreted_text" @markup.strong
  (role) @_role)
  (#eq? @_role ":strong:"))

[
  (inline_target)
  (substitution_reference)
  (footnote_reference)
  (citation_reference)
  (reference)
] @markup.link

; Others
[
  (title)
  "adornment"
] @markup.heading

(comment) @comment

(comment
  "..") @comment

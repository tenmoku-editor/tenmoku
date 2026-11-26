;; From nvim-treesitter's runtime/queries/rst/injections.scm at
;; 52773f0d13a0c11d0c3ef33c14ab6ecc1c17148b (https://github.com/nvim-treesitter/nvim-treesitter),
;; Apache-2.0 licence (see LICENSE-APACHE).
;;
;; Changes: the injection of comments into `comment` (not a language here) is
;; dropped. The three patterns injecting directive bodies as reStructuredText
;; are one, which injects the whole `body` with its children, and only when
;; the body doesn't start with arguments. Upstream injected the `content`
;; node, or (for admonitions) the `body` less its children, which is only
;; whitespace. But the grammar's scanner counts the indentation of a line
;; from the layer's own text: a `content` layer's first line has none and
;; the lines after it some, so a two-line body read as a definition list.
;; The whole body keeps each line's indentation, and reads as one block
;; quote. After arguments (`.. topic:: Title`) the body would be a
;; paragraph and a block quote, so it stays plain. The other injections are
;; into built-in languages (python, csv, and whatever a `code-block` or
;; `raw` argument names) and LaTeX, a first-party grammar.

((doctest_block) @injection.content
  (#set! injection.language "python"))

; Directives with nested content. The whole body is the layer, so each line
; keeps its indentation; a body starting with arguments stays plain.
((directive
  name: (type) @_type
  body: (body
    .
    [
      (options)
      (content)
    ]) @injection.content)
  (#set! injection.language "rst")
  (#set! injection.include-children)
  (#any-of? @_type
    "attention" "caution" "danger" "error" "hint" "important" "note" "tip" "warning" "admonition"
    "line-block" "parsed-literal" "epigraph" "highlights" "pull-quote" "compound" "header" "footer"
    "meta" "replace" "figure" "topic" "sidebar" "container" "table" "list-table" "class" "role"
    "restructuredtext-test-directive"))

; Special directives
((directive
  name: (type) @_type
  body: (body
    (arguments) @injection.language
    (content) @injection.content))
  (#any-of? @_type "raw" "code" "code-block" "sourcecode"))

((directive
  name: (type) @_type
  body: (body
    (content) @injection.content))
  (#set! injection.language "latex")
  (#eq? @_type "math"))

((directive
  name: (type) @_type
  body: (body
    (content) @injection.content))
  (#set! injection.language "csv")
  (#eq? @_type "csv-table"))

; Special roles - prefix
((interpreted_text
  (role) @_role
  "interpreted_text" @injection.content)
  (#eq? @_role ":math:")
  (#set! injection.language "latex"))

; Special roles - suffix
((interpreted_text
  "interpreted_text" @injection.content
  (role) @_role)
  (#eq? @_role ":math:")
  (#set! injection.language "latex"))

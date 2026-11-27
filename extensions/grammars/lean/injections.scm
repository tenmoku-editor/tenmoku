; From tree-sitter-lean at f35fa9b1ad3edd22c2b07b61f569a6d9c8e07d6e,
; https://github.com/Julian/tree-sitter-lean, path queries/injections.scm,
; license MIT. Changes: injects into a "/-!" comment that isn't at the top of
; the file too, which the grammar takes for a plain block comment.

; Doc comments are Markdown.
((doc_comment) @injection.content
  (#set! injection.language "markdown"))

((module_doc_comment) @injection.content
  (#set! injection.language "markdown"))

((block_comment) @injection.content
  (#match? @injection.content "^/-!")
  (#set! injection.language "markdown"))

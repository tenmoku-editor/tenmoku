; From tree-sitter-dot at 80327abbba6f47530edeb0df9f11bd5d5c93c14d,
; https://github.com/rydesun/tree-sitter-dot, path queries/injections.scm,
; license MIT. Unchanged: the text of an HTML-like label, `<...>`, is HTML.

((html_internal) @injection.content
  (#set! injection.language "html"))

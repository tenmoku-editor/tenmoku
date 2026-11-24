; Adapted from dont-be-evil-company/tree-sitter-kulala-http's
; queries/kulala_http/injections.scm at commit
; 1e238f6d34cf1b8c9cecedf78c073d8c6a3c8723 (MIT, the grammar's own licence
; and holders, text in LICENSE):
; https://github.com/dont-be-evil-company/tree-sitter-kulala-http/blob/1e238f6d34cf1b8c9cecedf78c073d8c6a3c8723/queries/kulala_http/injections.scm
;
; Changes: this header; the `comment` injection and the `kulala-jq`
; metadata's `jq` injection are dropped (no such languages here), and the
; script pattern's unused @_script capture with them. A body's `{{name}}`
; variables are named children, so they stay the host's, out of the
; injected JSON and GraphQL.

; Body
((json_body) @injection.content
  (#set! injection.language "json"))

((xml_body) @injection.content
  (#set! injection.language "xml"))

((graphql_data) @injection.content
  (#set! injection.language "graphql"))

; Inline script (JavaScript only)
((script
  (script_body) @injection.content)
  (#set! injection.include-children)
  (#set! injection.language "javascript"))

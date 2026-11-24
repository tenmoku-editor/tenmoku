; Adapted from dont-be-evil-company/tree-sitter-kulala-http's
; queries/kulala_http/highlights.scm at commit
; 1e238f6d34cf1b8c9cecedf78c073d8c6a3c8723 (MIT, the grammar's own licence
; and holders, text in LICENSE):
; https://github.com/dont-be-evil-company/tree-sitter-kulala-http/blob/1e238f6d34cf1b8c9cecedf78c073d8c6a3c8723/queries/kulala_http/highlights.scm
;
; Changes: this header; patterns reordered for last-pattern-wins (general
; before specific), with the comment and separator pattern first (upstream's
; came last, and would hide a separator's name); captures mapped to
; fe-syntax's vocabulary: header names are @property (were @constant), the
; `@` of declarations and metadata @punctuation.special (were
; @character.special), metadata names @attribute and values @string (were
; @keyword and @constant), query and form parameter names @property and
; values @string (were @query_param.* and @form_param_*), the URL
; @markup.link.url (was @string.special.url), a separator's name
; @markup.heading (was @keyword), a command's name @keyword and its target
; @string.special (were @function.method and @keyword), and body, redirect
; and script paths @string.special (were @external_body_path, @redirect_path
; and @number.special.path); @spell dropped. Added: a whole metadata line,
; whose `#` is an anonymous token, is @comment under its own captures; a
; multipart part's `< path` is @string.special like a body's; a
; `{{$name}}` dynamic variable is @variable.builtin; `<` and the `>>`
; redirect are @punctuation.special. Last pattern wins.

; Comments and request separators
[
  (comment)
  (request_separator)
  (metadata)
] @comment

(request_separator
  value: (_) @markup.heading)

; Variables
(identifier) @variable

((identifier) @variable.builtin
  (#match? @variable.builtin "^\\$"))

(variable_declaration
  "@" @punctuation.special)

(variable_declaration
  (value) @string)

(variable_declaration_inline
  "@" @punctuation.special)

(variable_declaration_inline
  (value) @string)

; Metadata
(metadata
  "@" @punctuation.special
  name: (_) @attribute)

(metadata
  value: (_) @string)

; Methods
(method) @function.method

; The request line
(request
  url: (_) @markup.link.url)

(query_param
  name: (_) @property)

(query_param
  value: (_) @string)

(fragment) @string.special

(http_version) @string.special

; Headers
(header
  name: (_) @property)

(header
  value: (_) @string)

(header
  ":" @punctuation.delimiter)

; Commands
(command
  name: (_) @keyword
  value: (_) @string.special)

; Operators
(metadata
  "=" @operator)

(variable_declaration
  "=" @operator)

(variable_declaration_inline
  "=" @operator)

(operator) @operator

; Bodies
(form_param
  name: (_) @property)

(form_param
  value: (_) @string)

(external_body
  path: (_) @string.special)

(multipart_external_body
  path: (_) @string.special)

(multipart_boundary_first) @keyword

(multipart_boundary) @keyword

(multipart_boundary_last) @keyword

; Redirects
(res_redirect) @punctuation.special

(res_redirect
  path: (_) @string.special)

; Scripts
(pre_request_script
  (path) @string.special)

(res_handler_script
  (path) @string.special)

"<" @punctuation.special

">" @punctuation.special

; Response
(status_code) @number

(status_text) @string

; Punctuation
[
  "{{"
  "}}"
  "{%"
  "%}"
  (interpolation_open)
  (interpolation_close)
] @punctuation.bracket

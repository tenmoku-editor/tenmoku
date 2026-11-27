; From tree-sitter-meson's queries/highlights.scm at
; aa8d472034956f94f51f2ef2cbfec4cc07efbfde,
; https://github.com/tree-sitter-grammars/tree-sitter-meson, MIT licence (see
; LICENSE).
;
; Changes: a variable pattern first, for every other identifier; the missing
; operators ("or", "in", the comparisons, "-=", "/=", "/" and the ternary's
; "?" and ":"); delimiters; string escapes; and patterns for method calls and
; properties. Later patterns win.

; Added: every identifier no pattern below names.
(identifier) @variable

(comment) @comment
(number) @number
(bool) @boolean

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

; Added.
[
  ","
  "."
  ":"
] @punctuation.delimiter

[
  "="
  "=="
  "and"
  "+"
  "!="
  "+="
  "not"
  ; Added from here.
  "or"
  "in"
  ">"
  "<"
  ">="
  "-="
  "/="
  "/"
  "?"
] @operator

; Added.
(ternaryoperator
  ":" @operator)

[
  "if"
  "elif"
  "else"
  "endif"
] @conditional

[
  "foreach"
  "endforeach"
  (keyword_break)
  (keyword_continue)
] @repeat

;;; format
(string) @string
["@"] @keyword

; Added.
(escape_sequence) @string.escape

(expression_statement
  object: (identifier) @variable)

(normal_command
  command: (identifier) @function)

; Added: a call after a ".", including the first call of a longer chain, which
; the grammar nests (`meson.project_version().split('.')`).
(expression_statement
  "."
  .
  (normal_command
    command: (identifier) @function.method))

(expression_statement
  "."
  .
  (expression_statement
    .
    (normal_command
      command: (identifier) @function.method)))

; Added.
(expression_statement
  property: (identifier) @property)

(pair
  key: (identifier) @property)

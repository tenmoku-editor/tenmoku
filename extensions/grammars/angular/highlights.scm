; From tree-sitter-angular at 38a8014ed5452cd6b7cf1399c00177a1f5374256,
; https://github.com/dlvandenberg/tree-sitter-angular, path
; queries/highlights.scm, license MIT. Upstream's query starts
; `; inherits: html`: in its place come tree-sitter-html 0.23.2's
; queries/highlights.scm (MIT, https://github.com/tree-sitter/tree-sitter-html
; at 5a5ca8551a179998360b4a4ca2c0f366a35acc03, tag v0.23.2; its licence, with
; its copyright notice, is in LICENSE-TREE-SITTER-HTML), whose grammar
; Angular's extends, unchanged. In Neovim's style, a later
; pattern wins (earlier_patterns_win = false), so Angular's patterns override
; HTML's and the general `(identifier) @variable` on the same node.
; Changes: that inlined HTML query, and the template reference pattern's
; `#lua-match? "#.*"`, which fe-syntax doesn't have, is `#match? "^#"`.

; tree-sitter-html 0.23.2, queries/highlights.scm.

(tag_name) @tag
(erroneous_end_tag_name) @tag.error
(doctype) @constant
(attribute_name) @attribute
(attribute_value) @string
(comment) @comment

[
  "<"
  ">"
  "</"
  "/>"
] @punctuation.bracket

; tree-sitter-angular, queries/highlights.scm.

(identifier) @variable

(style_unit) @variable

(pipe_operator) @operator

(string) @string

(number) @number

(pipe_call
  name: (identifier) @function)

(pipe_call
  arguments: (pipe_arguments
    (identifier) @variable.parameter))

(structural_directive
  "*" @keyword
  (identifier) @keyword)

(attribute
  (attribute_name) @variable.member
  (#match? @variable.member "^#"))

(binding_name
  (identifier) @keyword)

(class_binding
  [
    (identifier)
    (class_name)
  ] @keyword)

(event_binding
  (binding_name
    (identifier) @keyword))

(event_binding
  "\"" @punctuation.delimiter)

(property_binding
  [
   "\""
   "\"\""
  ] @punctuation.delimiter)

(structural_assignment
  operator: (identifier) @keyword)

(member_expression
  property: (identifier) @property)

(call_expression
  function: (identifier) @function)

(call_expression
  function: ((identifier) @function.builtin
    (#eq? @function.builtin "$any")))

(pair
  key: ((identifier) @variable.builtin
    (#eq? @variable.builtin "$implicit")))

[
  (control_keyword)
  (special_keyword)
] @keyword

((control_keyword) @keyword.repeat
  (#any-of? @keyword.repeat "for" "empty"))

((control_keyword) @keyword.conditional
  (#any-of? @keyword.conditional "if" "else" "switch" "case" "default"))

((control_keyword) @keyword.coroutine
  (#any-of? @keyword.coroutine "defer" "placeholder" "loading"))

((control_keyword) @keyword.exception
  (#eq? @keyword.exception "error"))

((identifier) @boolean
  (#any-of? @boolean "true" "false"))

((identifier) @variable.builtin
  (#any-of? @variable.builtin "this" "$event"))

((identifier) @constant.builtin
  (#eq? @constant.builtin "null"))

[
  (ternary_operator)
  (conditional_operator)
] @keyword.conditional.ternary

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "@"
] @punctuation.bracket

(two_way_binding
  [
    "[("
    ")]"
  ] @punctuation.bracket)

[
  "{{"
  "}}"
] @punctuation.special

(template_substitution
  [
    "${"
    "}"
  ] @punctuation.special)

(template_chars) @string

[
  ";"
  "."
  ","
  "?."
  "!."
] @punctuation.delimiter

(nullish_coalescing_expression
  (coalescing_operator) @operator)

(concatenation_expression
  "+" @operator)

(icu_clause) @keyword.operator

(icu_category) @keyword.conditional

(binary_expression
  [
    "-"
    "&&"
    "+"
    "<"
    "<="
    "="
    "=="
    "==="
    "!="
    "!=="
    ">"
    ">="
    "*"
    "/"
    "||"
    "%"
  ] @operator)

(arrow_function
  "=>" @operator)

(object
  (spread
    "..." @operator))

(array
  (spread
    "..." @operator))

(arguments
  (spread
    "..." @operator))

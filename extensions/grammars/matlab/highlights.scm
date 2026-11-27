; From tree-sitter-matlab, https://github.com/acristoffers/tree-sitter-matlab,
; commit 1bdb8cf5e73620892a199f659ceb9c06d02e4b2f, path
; queries/neovim/highlights.scm, license MIT (LICENSE). Later patterns win, as
; in Neovim. Changes:
; - #lua-match? is written as #match?, and its UpperCamelCase type pattern
;   comes straight after the plain variable one, so that properties, fields,
;   functions and parameters so named keep their own colour.
; - @spell, the ERROR pattern and the repeated comment pattern are dropped.
; - "arguments", "enumeration", "events", "methods" and "properties" are
;   keywords only where they open a block, not in a name.
; - "~" is an operator in a negation, and a built-in constant only as an
;   ignored argument.
; - "'" is an operator only as a transpose, not as a string's quote.
; - The `import` pattern comes after the command one, so that it wins.
; - Added: spmd, get. and set., `end` in an index, a lone ":", ".-", ".?",
;   the ";" between the rows of a matrix or cell (the grammar names no node
;   for it), superclasses, a property's class and an anonymous function's
;   parameters.

; Matrix and cell literals, first, so that every other capture of a node
; beats these. The ";" between rows is a token no query can name: it takes the
; literal's colour, which each row replaces with the plain one of @embedded.

(row) @embedded

[ (matrix) (cell) ] @punctuation.delimiter

; Keywords

[
  "classdef"
  "end"
  "global"
  "persistent"
] @keyword

(arguments_statement "arguments" @keyword)
(enumeration "enumeration" @keyword)
(events "events" @keyword)
(methods "methods" @keyword)
(properties "properties" @keyword)

(spmd_statement "spmd" @keyword)

[
  "get."
  "set."
] @keyword

(end_keyword) @keyword

; Conditionals

(if_statement [ "if" "end" ] @conditional)
(elseif_clause "elseif" @conditional)
(else_clause "else" @conditional)
(switch_statement [ "switch" "end" ] @conditional)
(case_clause "case" @conditional)
(otherwise_clause "otherwise" @conditional)
(break_statement) @conditional

; Repeats

(for_statement [ "for" "parfor" "end" ] @repeat)
(while_statement [ "while" "end" ] @repeat)
(continue_statement) @repeat

; Exceptions

(try_statement [ "try" "end" ] @exception)
(catch_clause "catch" @exception)

; Variables

(identifier) @variable

; Types, by their name's shape

((identifier) @type
  (#match? @type "^_*[A-Z][a-zA-Z0-9_]+$"))

; Constants

(events (identifier) @constant)
(attribute (identifier) @constant)

(ignored_argument) @constant.builtin

; Fields/Properties

(field_expression field: (identifier) @field)

(superclass "." (identifier) @property)

(property_name "." (identifier) @property)

(property name: (_) (identifier) @type)

(property name: (identifier) @property)

; Types

(class_definition name: (identifier) @type)

(superclasses (property_name (identifier) @type))

(attributes (identifier) @constant)

(enum . (identifier) @type)

; Functions

(function_definition
  "function" @keyword.function
  name: (identifier) @function
  [ "end" "endfunction" ]? @keyword.function)

(function_signature name: (identifier) @function)

(function_call
  name: (identifier) @function.call)

(handle_operator (identifier) @function)

(validation_functions (identifier) @function)

(command (command_name) @function.call)
(command_argument) @parameter

; Includes

((command_name) @include
  (#eq? @include "import"))

(return_statement) @keyword.return

; Parameters

(function_arguments (identifier) @parameter)

(lambda (arguments (identifier) @parameter))

; Punctuation

[ ";" "," "." ] @punctuation.delimiter

[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket

; Operators

[
  "+"
  ".+"
  "-"
  ".-"
  "*"
  ".*"
  "/"
  "./"
  "\\"
  ".\\"
  "^"
  ".^"
  "|"
  "&"
  "?"
  ".?"
  "@"
  "<"
  "<="
  ">"
  ">="
  "=="
  "~="
  "="
  "&&"
  "||"
  ":"
] @operator

(spread_operator) @operator

(not_operator "~" @operator)

(postfix_operator [ "'" ".'" ] @operator)

; Literals

(string) @string

(escape_sequence) @string.escape
(formatting_sequence) @string.special

(number) @number

((identifier) @boolean
  (#eq? @boolean "true"))
((identifier) @boolean
  (#eq? @boolean "false"))

; Comments

[ (comment) (line_continuation) ] @comment

; From tree-sitter-luau, https://github.com/polychromatist/tree-sitter-luau,
; commit 71b03e66b2c8dd04e0133c9b998a54a58f239ca4, path
; _archive/nvim-queries/highlights.scm, license MIT (see LICENSE). Later
; patterns win.
;
; Changes, mostly because that file predates the grammar's current node names:
; - this header;
; - a first `(name) @variable`, so every other name pattern overrides it;
; - node and field names follow the current grammar (`table_name`,
;   `variable_name`, `proptype`, `genericdef`, ...); the `op:` field is gone,
;   so "<" and ">" are operators, then brackets where they hold generics;
; - `@repeat`, `@conditional`, `@field`, `@parameter`, `@method`,
;   `@method.call` and `@namespace` are written in fe-syntax's names;
;   `@variable.global` is `@variable.builtin`; generics are `@type.parameter`,
;   not `@type.qualifier`, which colours as a keyword;
; - `#lua-match?` is `#match?`;
; - `@spell` and `(ERROR) @error` are dropped;
; - the per-library lists of standard functions are one pattern for any
;   function of a standard library, and the libraries are `@module.builtin`;
;   `buffer` and `vector` are added to the libraries and the built-in types;
; - a `@constant` name needs two characters or more;
; - built-in global functions are matched only where they're called;
; - strings are captured part by part in interpolated strings, with their
;   braces `@punctuation.special`;
; - `...` is `@variable.builtin`, not `@constant`;
; - added: attributes and the keys of their tables, escapes, `---` and `--@`
;   documentation comments, "//", "//=", "..=", "<<", ">>", "?", the "..." of
;   a type pack, `typeof`, the `read` and `write` modifiers, type functions and
;   the names in function types.

; Names
(name) @variable

; Constants
((name) @constant
  (#match? @constant "^[A-Z][A-Z_0-9]+$"))

; Keywords
"return" @keyword.return

[
  "in"
  "local"
] @keyword

(break_stmt) @keyword

(continue_stmt) @keyword

(do_stmt
  [
    "do"
    "end"
  ] @keyword)

(while_stmt
  [
    "while"
    "do"
    "end"
  ] @keyword.repeat)

(repeat_stmt
  [
    "repeat"
    "until"
  ] @keyword.repeat)

(for_in_stmt
  [
    "for"
    "do"
    "end"
  ] @keyword.repeat)

(for_range_stmt
  [
    "for"
    "do"
    "end"
  ] @keyword.repeat)

(if_stmt
  [
    "if"
    "then"
    "end"
  ] @keyword.conditional)

(elseif_clause
  [
    "elseif"
    "then"
  ] @keyword.conditional)

(else_clause
  "else" @keyword.conditional)

(ifexp
  [
    "if"
    "then"
    "elseif"
    "else"
  ] @keyword.operator)

(fn_stmt
  [
    "function"
    "end"
  ] @keyword.function)

(local_fn_stmt
  [
    "function"
    "end"
  ] @keyword.function)

(anon_fn
  [
    "function"
    "end"
  ] @keyword.function)

(type_fn_stmt
  [
    "function"
    "end"
  ] @keyword.function)

(type_stmt
  [
    "export"
    "type"
  ] @keyword)

(type_fn_stmt
  [
    "export"
    "type"
  ] @keyword)

[
  (read)
  (write)
] @keyword.modifier

; Operators
[
  "and"
  "not"
  "or"
  "typeof"
] @keyword.operator

[
  "+"
  "-"
  "*"
  "/"
  "//"
  "%"
  "^"
  "#"
  "=="
  "~="
  "<="
  ">="
  "<"
  ">"
  "="
  "&"
  "|"
  "+="
  "-="
  "*="
  "/="
  "//="
  "%="
  "^="
  "..="
  "->"
  "::"
  ".."
] @operator

(option) @operator

; The "..." of a generic or variadic type pack.
"..." @punctuation.special

; Punctuation
[
  ";"
  ":"
  ","
  "."
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "<<"
  ">>"
] @punctuation.bracket

(namedtype
  [
    "<"
    ">"
  ] @punctuation.bracket)

(type_stmt
  [
    "<"
    ">"
  ] @punctuation.bracket)

(fn_stmt
  [
    "<"
    ">"
  ] @punctuation.bracket)

(local_fn_stmt
  [
    "<"
    ">"
  ] @punctuation.bracket)

(anon_fn
  [
    "<"
    ">"
  ] @punctuation.bracket)

(type_fn_stmt
  [
    "<"
    ">"
  ] @punctuation.bracket)

(fntype
  [
    "<"
    ">"
  ] @punctuation.bracket)

; Variables
((var
  variable_name: (name) @variable.builtin)
  (#any-of? @variable.builtin "self" "_G" "_VERSION"))

((var
  table_name: (name) @variable.builtin)
  (#any-of? @variable.builtin "self" "_G"))

((var
  table_name: (name) @module.builtin)
  (#any-of? @module.builtin
    "bit32" "buffer" "coroutine" "debug" "math" "os" "string" "table" "task" "utf8" "vector"))

(vararg) @variable.builtin

; Literals
(nil) @constant.builtin

(boolean) @boolean

(number) @number

; Tables
(field
  field_name: (name) @property)

(key
  field_name: (name) @property)

(table
  [
    "{"
    "}"
  ] @constructor)

; Parameters
(param
  parameter_name: (name) @variable.parameter)

(paramtypelist
  (name) @variable.parameter)

; Functions
(call_stmt
  invoked: (var
    variable_name: (name) @function.call))

(call_stmt
  invoked: (var
    (key
      field_name: (name) @function.call)))

(call_stmt
  method_name: (name) @function.method.call)

((call_stmt
  invoked: (var
    variable_name: (name) @function.builtin))
  (#any-of? @function.builtin
    "assert" "collectgarbage" "error" "gcinfo" "getfenv" "getmetatable" "ipairs"
    "loadstring" "newproxy" "next" "pairs" "pcall" "print"
    "rawequal" "rawget" "rawlen" "rawset" "require" "select" "setfenv" "setmetatable"
    "tonumber" "tostring" "type" "typeof" "unpack" "xpcall"))

((call_stmt
  invoked: (var
    table_name: (name) @_library
    (key
      field_name: (name) @function.builtin)))
  (#any-of? @_library
    "bit32" "buffer" "coroutine" "debug" "math" "os" "string" "table" "task" "utf8" "vector"))

((var
  table_name: (name) @_library
  (key
    field_name: (name) @constant.builtin))
  (#eq? @_library "math")
  (#any-of? @constant.builtin "huge" "pi"))

(local_fn_stmt
  function_name: (name) @function)

(fn_stmt
  function_name: (name) @function)

; The last name of `function a.b.c`.
(fn_stmt
  (key
    field_name: (name) @function .))

(fn_stmt
  method_name: (name) @function.method)

(type_fn_stmt
  function_name: (name) @function)

; Types
(type_stmt
  left: (name) @type.definition)

(namedtype
  type_name: (name) @type)

(namedtype
  module_namespace: (name) @module)

((namedtype
  !module_namespace
  type_name: (name) @type.builtin)
  (#any-of? @type.builtin
    "any" "boolean" "buffer" "never" "number" "string" "thread" "unknown" "userdata" "vector"))

(generic
  (name) @type.parameter)

(genpack
  generic_typepack_name: (name) @type.parameter)

(proptype
  field_name: (name) @property)

; A key in an attribute's table.
(litfield
  (name) @property)

; Attributes
(attribute) @attribute

(parattr
  attribute_name: (name) @attribute)

; Strings
(string) @string

[
  (interp_start)
  (interp_content)
  (interp_end)
] @string

[
  (interp_brace_open)
  (interp_brace_close)
] @punctuation.special

[
  (simple_escape)
  (unicode_escape)
  (dec_byte_escape)
  (hex_byte_escape)
] @string.escape

; Comments
(comment) @comment

((comment) @comment.documentation
  (#match? @comment.documentation "^---"))

((comment) @comment.documentation
  (#match? @comment.documentation "^--\\s?@"))

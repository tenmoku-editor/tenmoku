; Adapted from tree-sitter-cython's queries/highlights.scm at
; https://github.com/b0o/tree-sitter-cython, commit
; 756a20c4d601b715f7b321eaad144f4cb14031bf, license MIT (LICENSE).
; Changes: `@escape` is `@string.escape` and `@embedded` is dropped;
; attributes come before calls, so that a method call isn't a property;
; `(decorator)` is captured by its `@` and name, not whole; added patterns for
; type names (class, struct, union, enum, cppclass, fused and ctypedef), a
; named buffer option such as `ndim=2`, `gil`/`nogil` in `with`, the dot,
; conversion and format in an f-string field, `?`, and the keywords `DEF`,
; `IF`, `ELIF`, `ELSE`, `api`, `by`, `include`, `new`, `noexcept`,
; `packed`, `sizeof`, `volatile`; dropped the pattern making a C-typed name a
; plain variable, which hid the constant colour of `cdef int MAX_ITER`.
; Later patterns win.

; Identifier naming conventions

(identifier) @variable

((identifier) @constructor
 (#match? @constructor "^[A-Z]"))

((identifier) @constant
 (#match? @constant "^[A-Z][A-Z_]*$"))

; Attributes

(attribute attribute: (identifier) @property)

; Function calls

(call
  function: (attribute attribute: (identifier) @function.method))
(call
  function: (identifier) @function)

; Builtin functions

((call
  function: (identifier) @function.builtin)
 (#match?
   @function.builtin
   "^(abs|all|any|ascii|bin|bool|breakpoint|bytearray|bytes|callable|chr|classmethod|compile|complex|delattr|dict|dir|divmod|enumerate|eval|exec|filter|float|format|frozenset|getattr|globals|hasattr|hash|help|hex|id|input|int|isinstance|issubclass|iter|len|list|locals|map|max|memoryview|min|next|object|oct|open|ord|pow|print|property|range|repr|reversed|round|set|setattr|slice|sorted|staticmethod|str|sum|super|tuple|type|vars|zip|__import__)$"))

; Types

(maybe_typed_name
  type: ((_) @type))

(type
  (identifier) @type)

(c_type
  type: ((_) @type))
(c_type
  ((identifier) @type))
(c_type
  ((int_type) @type))

; Function definitions

(function_definition
  name: (identifier) @function)

(cdef_statement
  (cvar_def
    (maybe_typed_name
      name: ((identifier) @function))
    (c_function_definition)))

(cvar_decl
  (c_type
    ([(identifier) (int_type)]))
  (c_name
    ((identifier) @function))
  (c_function_definition))

; Literals

[
  (none)
] @constant.builtin

[
  (true)
  (false)
] @boolean

[
  (integer)
  (float)
] @number

(comment) @comment
(string) @string
(escape_sequence) @string.escape

(interpolation
  "{" @punctuation.special
  "}" @punctuation.special)

(interpolation
  (attribute "." @punctuation.delimiter))

[
  (type_conversion)
  (format_specifier)
] @punctuation.special

[
  "-"
  "-="
  "!="
  "*"
  "**"
  "**="
  "*="
  "/"
  "//"
  "//="
  "/="
  "&"
  "&="
  "%"
  "%="
  "^"
  "^="
  "+"
  "->"
  "+="
  "<"
  "<<"
  "<<="
  "<="
  "<>"
  "="
  ":="
  "=="
  ">"
  ">="
  ">>"
  ">>="
  "|"
  "|="
  "~"
  "@="
  "and"
  "in"
  "is"
  "not"
  "or"
 "@"
 "?"
] @operator

[
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "exec"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "lambda"
  "nonlocal"
  "pass"
  "print"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
  "match"
  "case"

  ; cython-specific
  "cdef"
  "cpdef"
  "ctypedef"
  "cimport"
  "nogil"
  "gil"
  "extern"
  "inline"
  "public"
  "readonly"
  "struct"
  "union"
  "enum"
  "fused"
  "property"
  "namespace"
  "cppclass"
  "const"
  "volatile"
  "packed"
  "api"
  "noexcept"
  "by"
  "include"
  "new"
  "sizeof"
  "DEF"
  "IF"
  "ELIF"
  "ELSE"
] @keyword

; `with nogil:` and `with gil:` parse as a `with` of a name

((with_item
  value: (identifier) @keyword)
 (#match? @keyword "^(nogil|gil)$"))

; Type names

(class_definition
  name: (identifier) @type)

(struct
  (c_identifier
    name: (identifier) @type))

(cppclass
  (c_identifier
    name: (identifier) @type))

(enum
  (identifier) @type)

(fused
  (identifier) @type)

(ctypedef_statement
  (cvar_decl
    (identifier) @type))

; A buffer's named option, such as `ndim=2`

(type_index
  (c_type
    type: (identifier) @variable.parameter)
  .
  "=")

; Decorators

(decorator
  "@" @function)

(decorator
  [
    (identifier) @function
    (attribute
      attribute: (identifier) @function)
    (call
      function: [
        (identifier) @function
        (attribute
          attribute: (identifier) @function)
      ])
  ])

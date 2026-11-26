; CUDA highlights, later patterns winning (earlier_patterns_win = false).
;
; tree-sitter-cuda extends tree-sitter-cpp, keeping its node names, and its
; own queries/highlights.scm starts `; inherits: cpp`. So this file is, in
; order:
;
; - The C and C++ part: this repository's extensions/grammars/cpp/highlights.scm
;   as of commit 9c054a3e90796f71562ae07bcd19ff1d61242b85, verbatim, which is
;   queries/highlights.scm of tree-sitter-c 0.24.2
;   (https://github.com/tree-sitter/tree-sitter-c at
;   b780e47fc780ddc8da13afa35a3f4ed5c157823d) followed by
;   queries/highlights.scm of tree-sitter-cpp 0.23.4
;   (https://github.com/tree-sitter/tree-sitter-cpp at
;   f41e1a044c8a84ea9fa8577fdd2eab92ec96de02), both MIT; their licence and
;   copyright are in LICENSE-TREE-SITTER-CPP. Every node name in it exists in
;   tree-sitter-cuda, a fork of tree-sitter-cpp 0.23.x, so nothing is
;   dropped.
; - The CUDA part: queries/highlights.scm of
;   tree-sitter-grammars/tree-sitter-cuda at
;   d58080a327756e4d1d16ec329ba7cb2048f6c6cd, under the grammar's MIT licence
;   (LICENSE), without its `; inherits: cpp` line.
; - Patterns marked "Ours" at the end: CUDA's other qualifiers as keywords,
;   its built-in variables, and the runtime API's commonest enumerators.

; ---------------------------------------------------------------- C and C++

(identifier) @variable

((identifier) @constant
 (#match? @constant "^[A-Z][A-Z\\d_]*$"))

"break" @keyword
"case" @keyword
"const" @keyword
"continue" @keyword
"default" @keyword
"do" @keyword
"else" @keyword
"enum" @keyword
"extern" @keyword
"for" @keyword
"if" @keyword
"inline" @keyword
"return" @keyword
"sizeof" @keyword
"static" @keyword
"struct" @keyword
"switch" @keyword
"typedef" @keyword
"union" @keyword
"volatile" @keyword
"while" @keyword

"#define" @keyword
"#elif" @keyword
"#else" @keyword
"#endif" @keyword
"#if" @keyword
"#ifdef" @keyword
"#ifndef" @keyword
"#include" @keyword
(preproc_directive) @keyword

"--" @operator
"-" @operator
"-=" @operator
"->" @operator
"=" @operator
"!=" @operator
"*" @operator
"&" @operator
"&&" @operator
"+" @operator
"++" @operator
"+=" @operator
"<" @operator
"==" @operator
">" @operator
"||" @operator

"." @delimiter
";" @delimiter

(string_literal) @string
(system_lib_string) @string

(null) @constant
(number_literal) @number
(char_literal) @number

(field_identifier) @property
(statement_identifier) @label
(type_identifier) @type
(primitive_type) @type
(sized_type_specifier) @type

(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function))
(function_declarator
  declarator: (identifier) @function)
(preproc_function_def
  name: (identifier) @function.special)

(comment) @comment
; Functions

(call_expression
  function: (qualified_identifier
    name: (identifier) @function))

(template_function
  name: (identifier) @function)

(template_method
  name: (field_identifier) @function)

(template_function
  name: (identifier) @function)

(function_declarator
  declarator: (qualified_identifier
    name: (identifier) @function))

(function_declarator
  declarator: (field_identifier) @function)

; Types

((namespace_identifier) @type
 (#match? @type "^[A-Z]"))

(auto) @type

; Constants

(this) @variable.builtin
(null "nullptr" @constant)

; Keywords

[
 "catch"
 "class"
 "co_await"
 "co_return"
 "co_yield"
 "constexpr"
 "constinit"
 "consteval"
 "delete"
 "explicit"
 "final"
 "friend"
 "mutable"
 "namespace"
 "noexcept"
 "new"
 "override"
 "private"
 "protected"
 "public"
 "template"
 "throw"
 "try"
 "typename"
 "using"
 "concept"
 "requires"
 "virtual"
] @keyword

; Strings

(raw_string_literal) @string

; ---------------------------------------------------------------- CUDA

[
  "<<<"
  ">>>"
] @punctuation.bracket

[
  "__host__"
  "__device__"
  "__global__"
  "__managed__"
  "__forceinline__"
  "__noinline__"
] @keyword.modifier

"__launch_bounds__" @keyword.modifier

; Ours: the memory-space and other qualifiers tree-sitter-cuda adds to C++'s
; type_qualifier and declaration modifiers, which upstream's query leaves
; plain (`__shared__ float tile[32];`). `__device__`, `__global__` and
; `__managed__` there are the tokens above.
[
  "__shared__"
  "__constant__"
  "__local__"
  "__tile__"
  "__tile_global__"
  "__grid_constant__"
] @keyword.modifier

; Ours: the built-in variables every kernel reads (`threadIdx.x`).
((identifier) @variable.builtin
  (#any-of? @variable.builtin
    "threadIdx" "blockIdx" "blockDim" "gridDim" "warpSize"))

; Ours: the runtime API's enumerators the language's own code most names,
; which the all-caps constant pattern above misses (`cudaMemcpyHostToDevice`).
((identifier) @constant
  (#any-of? @constant
    "cudaSuccess"
    "cudaMemcpyHostToHost" "cudaMemcpyHostToDevice" "cudaMemcpyDeviceToHost"
    "cudaMemcpyDeviceToDevice" "cudaMemcpyDefault"))

; From tree-sitter-devicetree at e78bf56f206cb47bee28a217423acb651e076848,
; https://github.com/joelspadin/tree-sitter-devicetree, path
; queries/highlights.scm, license MIT (the grammar's own licence). Upstream has
; no `; inherits:` line, so this is self-contained, and it is copied verbatim
; below this header: changes none. There are no predicates or directives to
; drop.
;
; The card asks whether @tag and @label need aliasing or dropping. Neither:
; both are first-class names in fe-syntax's NAMES
; (crates/fe-syntax/src/highlight.rs), so @tag on a unit_address and @label on
; a node, property or memory_reservation label resolve directly. Every other
; capture here -- keyword, operator, punctuation.delimiter,
; punctuation.bracket, function, property, variable, constant, comment -- is
; also a name, so nothing in this file needed changing.

[
    "/delete-node/"
    "/delete-property/"
    "/dts-v1/"
    "/incbin/"
    "/include/"
    "/memreserve/"
    "/omit-if-no-ref/"
    "#define"
    "#undef"
    "#include"
    "#if"
    "#elif"
    "#else"
    "#endif"
    "#ifdef"
    "#ifndef"
] @keyword

[
    "!"
    "~"
    "-"
    "+"
    "*"
    "/"
    "%"
    "||"
    "&&"
    "|"
    "^"
    "&"
    "=="
    "!="
    ">"
    ">="
    "<="
    ">"
    "<<"
    ">>"
] @operator

[
    ","
    ";"
] @punctuation.delimiter

[
    "("
    ")"
    "{"
    "}"
    "<"
    ">"
] @punctuation.bracket

(call_expression
    function: (identifier) @function)

(node
    label: (identifier) @label)

(property
    label: (identifier) @label)

(memory_reservation
    label: (identifier) @label)

(property
    name: (identifier) @property)

(identifier) @variable

(unit_address) @tag

(reference) @constant

(comment) @comment
; Added for Tenmoku, and the only change to upstream's file. Upstream's query
; names no literal at all, so in a devicetree source every string and every
; cell value -- the bulk of the file's content -- was plain text:
; `compatible = "acme,demo";` and `reg = <0x10000 0x1000>;` had colour only on
; the punctuation. These four patterns use the grammar's own node names from
; node-types.json. They are appended last, which is safe under
; earlier_patterns_win = true because no earlier pattern matches these nodes.
(string_literal) @string
(system_lib_string) @string
(integer_literal) @number
(byte_string_literal) @string.special

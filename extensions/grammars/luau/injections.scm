; No injections.
;
; The grammar (polychromatist/tree-sitter-luau, commit
; 71b03e66b2c8dd04e0133c9b998a54a58f239ca4, MIT) ships only empty injection
; queries for its editors. A `ffi.cdef` string can't be injected into `c`
; either: a string's content is no node of its own here, so the injected text
; would hold the quotes. Comments and string patterns have no language here.

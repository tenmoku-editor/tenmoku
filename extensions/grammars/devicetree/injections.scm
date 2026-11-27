; Devicetree injects nothing: tree-sitter-devicetree ships no injections query
; at e78bf56f206cb47bee28a217423acb651e076848. Its #include and #define come
; from the C preprocessor, but the grammar keeps their contents as plain
; tokens rather than a region another language could parse.

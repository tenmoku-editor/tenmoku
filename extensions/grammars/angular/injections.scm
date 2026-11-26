; From tree-sitter-angular at 38a8014ed5452cd6b7cf1399c00177a1f5374256,
; https://github.com/dlvandenberg/tree-sitter-angular, path
; queries/injections.scm, license MIT. Upstream's query is only
; `; inherits: html`: in its place come tree-sitter-html 0.23.2's
; queries/injections.scm (MIT, https://github.com/tree-sitter/tree-sitter-html
; at 5a5ca8551a179998360b4a4ca2c0f366a35acc03, tag v0.23.2; its licence, with
; its copyright notice, is in LICENSE-TREE-SITTER-HTML), unchanged,
; injecting `<script>` as javascript and `<style>` as css.
; Template expressions are parsed by the Angular grammar itself, so nothing
; else is injected.

; tree-sitter-html 0.23.2, queries/injections.scm.

((script_element
  (raw_text) @injection.content)
 (#set! injection.language "javascript"))

((style_element
  (raw_text) @injection.content)
 (#set! injection.language "css"))

; From tree-sitter-nu at 5a5f9c25f9c356f703a637bb3658248a29713684,
; https://github.com/nushell/tree-sitter-nu, path queries/nu/injections.scm,
; license MIT (see LICENSE).
;
; Changes: the injection of comments into `comment` (not a language here) is
; dropped. The others are kept as they are: into `regex`, a built-in
; language, and into `nu` itself, for the string of `nu -c`.

(command
  head: ((cmd_identifier) @_cmd
    (#match? @_cmd "^\\s*(find|parse|split|str)$"))
  flag: (_
    name: (_) @_flag
    (#any-of? @_flag "r" "regex"))
  .
  arg: (_
    (string_content) @injection.content
    (#set! injection.language "regex")))

(_
  opr: [
    "=~"
    "!~"
    "like"
    "not-like"
  ]
  rhs: (_
    (string_content) @injection.content
    (#set! injection.language "regex")))

(command
  head: (_) @_cmd
  (#any-of? @_cmd "nu" "$nu.current-exe")
  flag: (_
    name: (_) @_flag
    (#any-of? @_flag "c" "e" "commands" "execute"))
  .
  arg: (_
    (string_content) @injection.content
    (#set! injection.language "nu")))

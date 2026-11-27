; Written for Tenmoku from src/node-types.json and grammar.js of
; tree-sitter-wolfram, https://github.com/WolframResearch/tree-sitter-wolfram,
; commit f7e561abf9ca78c511e629290c7245dbfaf7a8f8, under the grammar's MIT
; licence (LICENSE). The grammar ships no queries, and none were copied from
; another editor. Later patterns win, as in Neovim. A symbol no pattern names
; is plain text.

; Punctuation and operators

["[" "]" "[[" "]]" "{" "}" "(" ")" "<|" "|>"] @punctuation.bracket

["," ";"] @punctuation.delimiter

[
  "!" "!!" "!=" "&" "&&" "'" "*" "**" "*=" "+" "++" "+=" "-" "--" "-=" "->"
  "." ".." "..." "/" "/*" "/." "//" "//." "//=" "//@" "/:" "/;" "/=" "/@" ":"
  "::" ":=" ":>" ";;" "<" "<->" "<<" "<=" "<>" "=" "=!=" "=." "==" "===" ">"
  ">=" ">>" ">>>" "?" "@" "@*" "@@" "@@@" "^" "^:=" "^=" "|" "|->" "||" "~"
  "~~"
] @operator

; Operators written as long names
[
  "\\[And]" "\\[Conjugate]" "\\[ConjugateTranspose]" "\\[CubeRoot]" "\\[Del]"
  "\\[DirectedEdge]" "\\[Divide]" "\\[Divides]" "\\[DivisionSlash]"
  "\\[Equivalent]" "\\[Exists]" "\\[ForAll]" "\\[Function]"
  "\\[HermitianConjugate]" "\\[Implies]" "\\[InvisibleTimes]" "\\[Minus]"
  "\\[MinusPlus]" "\\[Nand]" "\\[Nor]" "\\[Not]" "\\[NotExists]" "\\[Or]"
  "\\[PlusMinus]" "\\[RoundImplies]" "\\[Rule]" "\\[RuleDelayed]" "\\[Sqrt]"
  "\\[Square]" "\\[Times]" "\\[Transpose]" "\\[TwoWayRule]"
  "\\[UndirectedEdge]" "\\[Xnor]" "\\[Xor]"
] @operator

; Literals

(comment) @comment

(string) @string

[(integer) (real)] @number

[(file_name) (freeform_content)] @string.special

(named_character) @string.escape

; "=[...]" free-form input: its opening token has no node of its own.
(freeform_evaluate) @operator

; Messages: symbol::tag

(message_name) @string.special

(message_tag) @string.special

; Patterns, slots and earlier results

(pattern name: (symbol) @variable.parameter)

[
  (blank)
  (blank_default)
  (blank_sequence)
  (blank_null_sequence)
] @operator

[(slot) (slot_sequence)] @variable.parameter

(out) @variable.builtin

; Functions: a call's head, and the function of "f @ x", "f /@ list",
; "f @@ list", "x // f" and "a ~f~ b"

(call head: (symbol) @function)

(binary . (symbol) @function . ["@" "/@" "//@" "@@" "@@@"])

(binary "//" . (symbol) @function)

(tilde function: (symbol) @function)

; Control, scoping and package heads

((symbol) @keyword
  (#match? @keyword "^(If|Which|Switch|Do|For|While|Module|Block|With|Table|Map|Function|Print|Return|Throw|Catch|Check|Begin|End|BeginPackage|EndPackage|Needs|Get|Package)$"))

; Built-in constants and $ symbols

((symbol) @constant.builtin
  (#match? @constant.builtin "^([$].*|True|False|Null|None|All|Automatic|Infinity|Pi|E|I|Missing)$"))

; A named character such as \[Alpha] that's a letter, and so a symbol

((symbol) @string.escape
  (#match? @string.escape "^\\\\\\[[A-Za-z]+\\]$"))

; A script's "#!/..." line isn't Wolfram, but the grammar parses it as code:
; "#!" and then divisions.

(source_file . (_) @comment
  (#match? @comment "^#!/"))

(binary
  (postfix (slot) @comment "!" @comment)
  "/")

((binary "/" @comment) @_shebang
  (#match? @_shebang "^#!/"))

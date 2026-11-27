; From tree-sitter-lean at f35fa9b1ad3edd22c2b07b61f569a6d9c8e07d6e,
; https://github.com/Julian/tree-sitter-lean, path queries/highlights.scm,
; license MIT. Changes: "@[" and "]" are coloured as an attribute's only
; inside `attributes` (upstream's later pattern took every "]"); a "/-!"
; comment is documentation wherever it is; an UpperCamelCase name is a type,
; an applied one too. Added: the keywords upstream leaves out (`lemma`, the
; macro, notation and option commands, further `#` commands and tactic words);
; the do-notation words the grammar parses as identifiers (`for`, `in`,
; `return` and the like); bare tactics, binder names, type annotations,
; universe levels, `.ctor` names, `where` field names, "#[", "." and further
; operators ("×", "==", "<;>" and the like).

; Highlight queries for tree-sitter-lean.
;
; Capture names follow the nvim-treesitter capture convention, which maps
; back to Lean's SemanticTokenType vocabulary (see
;   lean4/src/Lean/Data/Lsp/LanguageFeatures.lean
; for the authoritative list). When the Lean LSP server is attached, its
; semantic tokens take precedence over these queries at runtime; the
; queries provide the floor for offline/cold-start highlighting.

; ---------- comments ------------------------------------------------------

(line_comment)        @comment
(block_comment)       @comment
(doc_comment)         @comment.documentation
(module_doc_comment)  @comment.documentation
; The grammar takes "/-!" for a module doc only at the top of a file.
((block_comment) @comment.documentation
  (#match? @comment.documentation "^/-!"))

; ---------- literals ------------------------------------------------------

(num_lit)             @number
(scientific_lit)      @number.float
(char_lit)            @character
(str_lit)             @string
(raw_string)          @string.special
(interpolated_str)    @string
(escape_sequence)     @string.escape
(name_lit)            @string.special.symbol

(true_const)          @boolean
(false_const)         @boolean

((sorry) @keyword.exception)    ; Lean tags `sorry`/`admit` as leanSorryLike

; ---------- type-level constants ------------------------------------------

(type_const)          @type.builtin
(sort_const)          @type.builtin
(prop_const)          @type.builtin

; ---------- identifiers ---------------------------------------------------

; Plain identifier — default to variable. More-specific queries below
; override for declaration names, types, etc.
(identifier)          @variable
; Lean names types and classes in UpperCamelCase.
((identifier) @type
  (#match? @type "^[A-Z][A-Za-z0-9]*$"))

; Holes & meta-variables.
(hole)                @variable.builtin
(synth_hole)          @variable.builtin
(named_hole (identifier) @variable.builtin)

; ---------- declaration names --------------------------------------------

(def       name: (identifier) @function)
(theorem   name: (identifier) @function)
(abbrev    name: (identifier) @function)
(opaque    name: (identifier) @function)
(constant  name: (identifier) @function)
(axiom     name: (identifier) @function)
(instance  name: (identifier) @function)

(structure name: (identifier) @type)
(inductive name: (identifier) @type)

(ctor       name: (identifier) @constructor)
(ctor_alt   name: (identifier) @constructor)

(field      name: (identifier) @property)

; Namespace / section names.
(namespace name: (identifier) @namespace)
(section   name: (identifier) @namespace)
(end       name: (identifier) @namespace)

; Universe names.
(universe  name: (identifier) @type.parameter)

; Import paths.
(import    name: (identifier) @module)
(open      namespace: (identifier) @module)
(open      only: (identifier)      @function)
(open      scoped: (identifier)    @module)
(export    namespace: (identifier) @module)
(export    only: (identifier)      @function)

; Attribute names.
(attributes name: (identifier) @attribute)

; Projection / field access.
(proj field: (identifier) @property)
(proj field: (num_lit)    @number)

; Named-argument labels.
(paren arg_name: (identifier) @variable.parameter)

; Function call: highlight the called name as @function when it's a
; bare identifier.
(app fn: (identifier) @function.call)
((app fn: (identifier) @type)
  (#match? @type "^[A-Z][A-Za-z0-9]*$"))

; ---------- keywords ------------------------------------------------------

; Module-system keywords. `module`/`prelude`/`public`/`meta`/`all` each
; have a same-named rule wrapping the literal, so we query the rules
; rather than the bare strings.
(moduleTk)  @keyword.import
(prelude)   @keyword.import
(public)    @keyword.import
(meta)      @keyword.import
(all)       @keyword.import
"import"    @keyword.import
(public_section) @keyword.import

[
  "namespace"
  "section"
  "end"
] @keyword

[
  "def"
  "theorem"
  "example"
  "abbrev"
  "instance"
  "axiom"
  "opaque"
  "constant"
  "structure"
  "inductive"
  "class"
] @keyword

[
  "variable"
  "universe"
  "universes"
  "open"
  "export"
  "extends"
  "deriving"
  "where"
] @keyword

[
  "noncomputable"
  "partial"
  "nonrec"
  "private"
  "protected"
  "unsafe"
  "mut"
  "scoped"
  "local"
] @keyword.modifier

[
  "#check"
  "#eval"
  "#print"
  "#reduce"
] @keyword.directive

[
  "fun"
  "λ"
] @keyword.function

[
  "let"
  "have"
  "show"
  "from"
] @keyword

[
  "if"
  "then"
  "else"
] @keyword.conditional

[
  "match"
  "with"
] @keyword.conditional

[
  "by"
  "do"
] @keyword

[
  "forall"
  "∀"
  "exists"
  "∃"
] @keyword.operator

; ---------- operators -----------------------------------------------------

[
  "→" "->" "↔" "<->" "↦" "=>"
  "∨" "||" "∧" "&&" "¬" "!"
  "=" "≠" "!=" "<" "≤" "<=" ">" "≥" ">="
  "∈" "∉" "⊆" "⊂" "⊇" "⊃"
  "≡" "≢" "~" "≃" "≅"
  "::" "++"
  "+" "-" "*" "/" "%"
  "∪" "∩" "\\"
  "^" "∘"
  "|>" "<|"
  ">>" "<<" ">>>" "<<<"
  ">>=" "=<<"
  "<$>" "<*>" "<|>" "<&>"
  "&&&" "|||" "^^^"
  "←" "<-"
] @operator

; ---------- punctuation ---------------------------------------------------

[":=" ":"] @operator
["," ";" "|"] @punctuation.delimiter
["(" ")" "{" "}" "[" "]" "⟨" "⟩" "⦃" "⦄" "{{" "}}"] @punctuation.bracket
(attributes ["@[" "]"] @attribute)

; ---------- additions (see the header) -----------------------------------

[
  "lemma"
  "macro"
  "macro_rules"
  "syntax"
  "declare_syntax_cat"
  "elab_rules"
  "notation"
  "infix"
  "infixl"
  "infixr"
  "prefix"
  "postfix"
  "attribute"
  "set_option"
  "register_option"
  "builtin_initialize"
  "initialize"
  "rec"
  "in"
  "for"
  "using"
  "generalizing"
  "case"
  "cases"
  "by_cases"
  "induction"
  "obtain"
  "set"
  "suffices"
  "letI"
  "haveI"
] @keyword

[
  "#check_failure"
  "#exit"
  "#print_axioms"
  "#synth"
  "#version"
] @keyword.directive

["×" "==" "<;>" "<&&>" "<||>" "//" "▸" "⊕" "↑"] @operator
"#[" @punctuation.bracket
"." @punctuation.delimiter

(explicit_binder        name: (identifier) @variable.parameter)
(implicit_binder        name: (identifier) @variable.parameter)
(strict_implicit_binder name: (identifier) @variable.parameter)
(instance_binder        name: (identifier) @variable.parameter)
(binders (identifier) @variable.parameter)

(_ type: (identifier) @type)
(_ type: (app fn: (identifier) @type))
(type_const (identifier) @type.parameter)
(sort_const (identifier) @type.parameter)

(dot_ident name: (identifier) @constructor)
(struct_field name: (identifier) @property)

; A tactic with no argument is a bare identifier.
(by stmt: (identifier) @function.call)
(by inline: (identifier) @function.call)

; Reserved words of do-notation, which the grammar has no rules for.
((identifier) @keyword
  (#any-of? @keyword
    "break" "catch" "continue" "finally" "for" "in" "return" "try" "unless"
    "while"))

; ---------- raw atoms ----------------------------------------------------

["`" "'" "\"" "s!\"" "m!\""] @string

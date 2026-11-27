; From tree-sitter-elisp at 0cbf0906d9ee707c8c109422fba9cdd17ae13dcf,
; https://github.com/Wilfred/tree-sitter-elisp, path queries/highlights.scm,
; license MIT (the grammar's own licence). Upstream has no `; inherits:` line,
; so this is self-contained, and it is copied verbatim below this header:
; changes none. Every capture is already in fe-syntax's vocabulary and its
; three #match? predicates are supported.
;
; Two notes on captures, one of which the card asked about. The card reports a
; capture `@string.` with a trailing dot: there is no such capture. That text
; is the last word of a comment sentence -- ";; Docstrings are strings too, so
; these come after (string) @string." -- where the dot is the full stop. There
; was nothing to drop or fix, so nothing was changed. Separately,
; @string.documentation is not one of fe-syntax's names:
; Highlight::from_capture strips dotted segments until one matches, so
; docstrings colour as `string`, like any other string. That loses a
; distinction rather than being a defect, and renaming the capture would lose
; the information for a later reader of this file.

;; When several patterns capture the same node, the last one wins, so
;; a pattern that refines another must come after it.

;; Special forms
[
  "and"
  "catch"
  "cond"
  "condition-case"
  "defconst"
  "defvar"
  "function"
  "if"
  "interactive"
  "lambda"
  "let"
  "let*"
  "or"
  "prog1"
  "prog2"
  "progn"
  "quote"
  "save-current-buffer"
  "save-excursion"
  "save-restriction"
  "setq"
  "setq-default"
  "unwind-protect"
  "while"
] @keyword

;; Function definitions
[
 "defun"
 "defsubst"
 ] @keyword
(function_definition name: (symbol) @function)
(function_definition parameters: (list (symbol) @variable.parameter))

;; Highlight macro definitions the same way as function definitions.
"defmacro" @keyword
(macro_definition name: (symbol) @function)
(macro_definition parameters: (list (symbol) @variable.parameter))

;; &optional and &rest are argument list markers, not parameters.
;; https://www.gnu.org/software/emacs/manual/html_node/elisp/Argument-List.html
(function_definition
  parameters: (list (symbol) @keyword
    (#match? @keyword "^&")))
(macro_definition
  parameters: (list (symbol) @keyword
    (#match? @keyword "^&")))

;; A sharp quoted symbol is a function reference, e.g. #'foo.
(function_quote (symbol) @function)

;; The variable defined by defvar or defconst. The anchor restricts
;; this to the first symbol, leaving the initial value alone.
(special_form
  [
    "defconst"
    "defvar"
  ]
  .
  (symbol) @variable)

;; Variables bound by let and let*, written either as (let ((x 1)))
;; or as (let (x)).
(special_form
  [
    "let"
    "let*"
  ]
  .
  (list
    [
      (symbol) @variable
      (list . (symbol) @variable)
    ]))

(comment) @comment

(integer) @number
(float) @number
;; Characters are integers in Emacs Lisp, e.g. ?a is 97.
(char) @number

(string) @string

;; Docstrings are strings too, so these come after (string) @string.
(function_definition docstring: (string) @string.documentation)
(macro_definition docstring: (string) @string.documentation)
;; defvar and defconst take the docstring after the initial value, so
;; (defvar foo nil "Doc.") has one but (defvar foo "Value") does not.
(special_form
  [
    "defconst"
    "defvar"
  ]
  .
  (symbol)
  .
  (_)
  .
  (string) @string.documentation)

;; #$ is the name of the file being loaded.
;; https://www.gnu.org/software/emacs/manual/html_node/elisp/Special-Read-Syntax.html
(byte_compiled_file_name) @constant.builtin

[
  "("
  ")"
  "#("
  "#["
  "["
  "]"
] @punctuation.bracket

[
  "`"
  "#'"
  "'"
  ","
  ",@"
] @operator

;; Highlight nil and t as constants, unlike other symbols
[
  "nil"
  "t"
] @constant.builtin

;; Keywords evaluate to themselves, so highlight them as constants too.
;; https://www.gnu.org/software/emacs/manual/html_node/elisp/Constant-Variables.html
((symbol) @constant
  (#match? @constant "^:"))

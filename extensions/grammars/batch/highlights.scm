; Adapted from https://github.com/wharflab/tree-sitter-batch
; queries/highlights.scm at 2aa22f00d7f5bbe58556b8eef0b1fc1629508603 (MIT).
; The dynamic-variable pattern moved after the generic variable one, since the
; later pattern wins here. `::` comments are also parsed by the grammar; the
; manifest's line_comment takes one token, REM.
; Echo off/on
(echo_off) @keyword

; Comments
(comment) @comment

; Labels
(label) @label

; Variable assignment
(set_keyword) @keyword
(variable_name) @variable
(set_option) @constant
(assignment_literal) @string
(arithmetic_expression) @string

; IF/FOR/GOTO/CALL statements. Their keywords are anonymous patterns that
; queries can't name, so the whole statement is a keyword; the later patterns
; below recolour its operands, and the statement's own text is only keywords,
; blanks, parentheses and a goto/call target.
(if_stmt) @keyword
(for_stmt) @keyword
(goto_stmt) @keyword
(call_stmt) @keyword
(setlocal_stmt) @keyword
(endlocal_stmt) @keyword
(exit_stmt) @keyword

; Operators
(comparison_op) @operator
(redirect_op) @operator
(fd_redirect) @operator

; Commands
(command_name) @function

; Variables
(variable_reference) @variable

; CMD pseudo/dynamic environment variables
((variable_reference) @variable.builtin
  (#match? @variable.builtin "(?i)^[%!](CD|ERRORLEVEL|DATE|TIME|RANDOM|CMDCMDLINE|CMDEXTVERSION|HIGHESTNUMANODENUMBER|__APPDIR__|__CD__)[%!]$"))

; FOR set literal content
(for_set_literal) @string

; FOR loop variables
(for_variable) @variable.parameter

; FOR options
(for_options) @constant

; Strings
(string) @string

; Numbers
(integer) @number

; Command options/flags
(command_option) @constant

; Argument values
(argument_value) @string

; Redirect targets
(redirect_target) @string.special

; Block parentheses
["(" ")"] @punctuation.bracket

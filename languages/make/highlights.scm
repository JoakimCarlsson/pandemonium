; Make: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(comment) @comment

((targets
  (word) @constant.builtin)
  (#match? @constant.builtin "^\\.(PHONY|SUFFIXES|DEFAULT|PRECIOUS|INTERMEDIATE|NOTINTERMEDIATE|SECONDARY|SECONDEXPANSION|DELETE_ON_ERROR|IGNORE|LOW_RESOLUTION_TIME|SILENT|EXPORT_ALL_VARIABLES|NOTPARALLEL|ONESHELL|POSIX)$"))
(targets
  (word) @function)

(automatic_variable) @variable.builtin

[
  "VPATH"
  ".RECIPEPREFIX"
] @variable.builtin
((variable_assignment
  name: (word) @variable.builtin)
  (#match? @variable.builtin "^(MAKEFLAGS|MAKEFILE_LIST|MAKECMDGOALS|CURDIR|SHELL|\\.DEFAULT_GOAL|\\.EXTRA_PREREQS|\\.VARIABLES|\\.FEATURES|\\.INCLUDE_DIRS)$"))
((variable_reference
  (word) @variable.builtin)
  (#match? @variable.builtin "^(MAKE|MAKEFLAGS|MAKEFILE_LIST|MAKECMDGOALS|CURDIR|SHELL|\\.DEFAULT_GOAL|\\.VARIABLES|\\.FEATURES|\\.INCLUDE_DIRS)$"))

(variable_assignment
  name: (word) @constant)
(shell_assignment
  name: (word) @constant)
(define_directive
  name: (word) @constant)
(undefine_directive
  variable: (word) @constant)
(variable_reference
  (word) @constant)
(substitution_reference
  (word) @constant)

(function_call
  function: _ @function.builtin)
(shell_function
  function: _ @function.builtin)

(string) @string
(raw_text) @string
(variable_assignment
  value: (text) @string)
(arguments
  (text) @string)

((prerequisites
  (word) @string.special)
  (#match? @string.special "%"))
((pattern_list
  (word) @string.special)
  (#match? @string.special "%"))

[
  "ifeq"
  "ifneq"
  "ifdef"
  "ifndef"
  "else"
  "endif"
  "define"
  "endef"
  "vpath"
  "undefine"
  "export"
  "unexport"
  "override"
  "private"
  "include"
  "sinclude"
  "-include"
] @keyword

(recipe_line
  [
    "@"
    "-"
    "+"
  ] @operator)

[
  "$"
  "$$"
] @punctuation.special

[
  "("
  ")"
  "{"
  "}"
] @punctuation.bracket

[
  ":"
  "&:"
  "::"
  "|"
  ";"
  ","
] @punctuation.delimiter

[
  "="
  ":="
  "::="
  "?="
  "+="
  "!="
] @operator

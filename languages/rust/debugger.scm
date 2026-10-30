; Rust: the names whose value is shown beside the line while the program is
; paused, and the blocks they are looked for in.

(let_declaration
  pattern: (identifier) @debug-variable)

(parameter
  pattern: (identifier) @debug-variable)

(self) @debug-variable

(for_expression
  pattern: (identifier) @debug-variable)

(assignment_expression
  left: (identifier) @debug-variable)

(field_expression
  value: (identifier) @debug-variable)

(binary_expression
  left: (identifier) @debug-variable
  (#not-match? @debug-variable "^[A-Z]"))

(binary_expression
  right: (identifier) @debug-variable
  (#not-match? @debug-variable "^[A-Z]"))

(arguments
  (identifier) @debug-variable
  (#not-match? @debug-variable "^[A-Z]"))

(return_expression
  (identifier) @debug-variable)

(block) @debug-scope

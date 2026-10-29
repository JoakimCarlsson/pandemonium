; Python: the names whose value is shown beside the line while the program is
; paused, and the blocks they are looked for in.

(assignment
  left: (identifier) @debug-variable)

(assignment
  left: (pattern_list
    (identifier) @debug-variable))

(augmented_assignment
  left: (identifier) @debug-variable)

(for_statement
  left: (identifier) @debug-variable)

(parameters
  (identifier) @debug-variable)

(binary_operator
  left: (identifier) @debug-variable
  (#not-match? @debug-variable "^[A-Z]"))

(binary_operator
  right: (identifier) @debug-variable
  (#not-match? @debug-variable "^[A-Z]"))

(return_statement
  (identifier) @debug-variable)

(block) @debug-scope

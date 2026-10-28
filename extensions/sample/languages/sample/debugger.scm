; Sample: the names whose value is shown beside the line while the program is
; paused, and the pieces of text they are looked for in.

(assignment
  left: (identifier) @debug-variable)

(augmented_assignment
  left: (identifier) @debug-variable)

(for_statement
  left: (identifier) @debug-variable)

(parameters
  (identifier) @debug-variable)

(binary_operator
  left: (identifier) @debug-variable)

(binary_operator
  right: (identifier) @debug-variable)

(return_statement
  (identifier) @debug-variable)

(block) @debug-scope

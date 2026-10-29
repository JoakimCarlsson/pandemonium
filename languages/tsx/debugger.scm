; TSX: the names whose value is shown beside the line while the program is
; paused, and the blocks they are looked for in.

(variable_declarator
  name: (identifier) @debug-variable)

(assignment_expression
  left: (identifier) @debug-variable)

(binary_expression
  left: (identifier) @debug-variable)

(binary_expression
  right: (identifier) @debug-variable)

(arguments
  (identifier) @debug-variable)

(return_statement
  (identifier) @debug-variable)

(statement_block) @debug-scope

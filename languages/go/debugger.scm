; Go: the names whose value is shown beside the line while the program is
; paused, and the blocks they are looked for in.

(short_var_declaration
  left: (expression_list
    (identifier) @debug-variable))

(var_spec
  name: (identifier) @debug-variable)

(parameter_declaration
  name: (identifier) @debug-variable)

(assignment_statement
  left: (expression_list
    (identifier) @debug-variable))

(binary_expression
  left: (identifier) @debug-variable)

(binary_expression
  right: (identifier) @debug-variable)

(argument_list
  (identifier) @debug-variable)

(return_statement
  (expression_list
    (identifier) @debug-variable))

(block) @debug-scope

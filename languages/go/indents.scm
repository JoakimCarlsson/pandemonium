; Go: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (block)
  (field_declaration_list)
  (interface_type)
  (literal_value)
  (argument_list)
  (parameter_list)
  (import_spec_list)
  (const_declaration)
  (var_declaration)
  (expression_switch_statement)
  (type_switch_statement)
  (select_statement)
  (expression_case)
  (type_case)
  (default_case)
  (communication_case)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

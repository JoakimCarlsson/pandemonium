; C++: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (compound_statement)
  (field_declaration_list)
  (declaration_list)
  (enumerator_list)
  (initializer_list)
  (case_statement)
  (argument_list)
  (parameter_list)
  (parenthesized_expression)
  (template_argument_list)
  (template_parameter_list)
  (lambda_capture_specifier)
  (requirement_seq)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

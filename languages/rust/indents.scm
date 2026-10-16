; Rust: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (block)
  (declaration_list)
  (field_declaration_list)
  (enum_variant_list)
  (match_block)
  (use_list)
  (field_initializer_list)
  (token_tree)
  (arguments)
  (parameters)
  (array_expression)
  (tuple_expression)
  (type_arguments)
  (type_parameters)
  (where_clause)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

; C#: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (declaration_list)
  (enum_member_declaration_list)
  (accessor_list)
  (block)
  (switch_body)
  (switch_section)
  (switch_expression)
  (initializer_expression)
  (anonymous_object_creation_expression)
  (collection_expression)
  (property_pattern_clause)
  (argument_list)
  (bracketed_argument_list)
  (parameter_list)
  (bracketed_parameter_list)
  (attribute_argument_list)
  (parenthesized_expression)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

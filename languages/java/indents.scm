; Java: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (class_body)
  (interface_body)
  (enum_body)
  (annotation_type_body)
  (module_body)
  (record_pattern_body)
  (constructor_body)
  (block)
  (switch_block)
  (array_initializer)
  (element_value_array_initializer)
  (argument_list)
  (formal_parameters)
  (annotation_argument_list)
  (resource_specification)
  (parenthesized_expression)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

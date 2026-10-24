; GraphQL: a node captured @indent indents the lines inside it; a token
; captured @outdent closes one and goes back to the level it opened at.

[
  (selection_set)
  (fields_definition)
  (input_fields_definition)
  (enum_values_definition)
  (arguments)
  (arguments_definition)
  (variable_definitions)
  (object_value)
  (list_value)
  (schema_definition)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

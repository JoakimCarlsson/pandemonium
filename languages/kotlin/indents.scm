; Kotlin: a node captured @indent indents the lines inside it; a token
; captured @outdent closes one and goes back to the level it opened at.

[
  (class_body)
  (enum_class_body)
  (function_body)
  (control_structure_body)
  (lambda_literal)
  (when_expression)
  (try_expression)
  (catch_block)
  (finally_block)
  (anonymous_initializer)
  (secondary_constructor)
  (getter)
  (setter)
  (value_arguments)
  (function_value_parameters)
  (primary_constructor)
  (collection_literal)
  (indexing_suffix)
  (parenthesized_expression)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

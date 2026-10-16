; Swift: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. The cases of a
; switch sit level with it, and what each case does goes one deeper.

[
  (class_body)
  (enum_class_body)
  (protocol_body)
  (function_body)
  (computed_property)
  (computed_getter)
  (computed_setter)
  (computed_modify)
  (willset_didset_block)
  (willset_clause)
  (didset_clause)
  (lambda_literal)
  (if_statement)
  (guard_statement)
  (for_statement)
  (while_statement)
  (repeat_while_statement)
  (do_statement)
  (catch_block)
  (switch_entry)
  (value_arguments)
  (tuple_expression)
  (tuple_type)
  (array_literal)
  (dictionary_literal)
  (type_parameters)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

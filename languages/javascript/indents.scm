; JavaScript: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (statement_block)
  (class_body)
  (object)
  (object_pattern)
  (array)
  (array_pattern)
  (arguments)
  (formal_parameters)
  (switch_body)
  (switch_case)
  (switch_default)
  (template_string)
  (parenthesized_expression)
  (named_imports)
  (export_clause)
  (jsx_element)
  (jsx_self_closing_element)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

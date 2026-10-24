; PowerShell: a node captured @indent indents the lines inside it; a token
; captured @outdent closes one and goes back to the level it opened at.

[
  (statement_block)
  (script_block)
  (param_block)
  (parenthesized_expression)
  (array_expression)
  (hash_literal_expression)
  (switch_body)
  (class_statement)
  (enum_statement)
] @indent

[
  "}"
  ")"
] @outdent

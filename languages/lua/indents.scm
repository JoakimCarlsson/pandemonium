; Lua: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. A block is
; closed by its end, or by until after repeat.

[
  (function_declaration)
  (function_definition)
  (if_statement)
  (elseif_statement)
  (else_statement)
  (for_statement)
  (while_statement)
  (repeat_statement)
  (do_statement)
  (table_constructor)
  (arguments)
  (parameters)
  (parenthesized_expression)
] @indent

[
  "end"
  "until"
  "else"
  "elseif"
  "}"
  ")"
  "]"
] @outdent

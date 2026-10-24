; Zig: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (block)
  (struct_declaration)
  (enum_declaration)
  (union_declaration)
  (opaque_declaration)
  (error_set_declaration)
  (switch_expression)
  (initializer_list)
  (parameters)
  (arguments)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

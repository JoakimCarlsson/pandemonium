; HCL: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (block)
  (object)
  (tuple)
  (for_object_expr)
  (for_tuple_expr)
  (function_call)
] @indent

[
  (block_end)
  (object_end)
  (tuple_end)
  ")"
  "]"
] @outdent

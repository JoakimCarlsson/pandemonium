; Protocol Buffers: a node captured @indent indents the lines inside it; a
; token captured @outdent closes one and goes back to the level it opened at.

[
  (message_body)
  (enum_body)
  (oneof)
  (service)
  (rpc)
  (block_lit)
] @indent

[
  "}"
  "]"
  ")"
] @outdent

(block_lit
  ">" @outdent)

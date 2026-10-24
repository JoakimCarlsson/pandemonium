; TOML: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (array)
  (inline_table)
] @indent

[
  "]"
  "}"
] @outdent

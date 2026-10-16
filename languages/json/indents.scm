; JSON: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (object)
  (array)
] @indent

[
  "}"
  "]"
] @outdent

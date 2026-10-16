; Dockerfile: a node captured @indent indents the lines inside it; a token
; captured @outdent closes one and goes back to the level it opened at.

[
  (json_string_array)
] @indent

"]" @outdent

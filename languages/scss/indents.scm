; SCSS: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (block)
  (keyframe_block_list)
  (arguments)
  (parameters)
] @indent

[
  "}"
  ")"
] @outdent

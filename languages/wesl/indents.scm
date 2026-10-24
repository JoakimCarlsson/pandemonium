; WESL: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (compound_statement)
  (struct_body)
  (switch_body)
  (import_collection)
  (param_list)
  (argument_list)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

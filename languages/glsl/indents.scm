; GLSL: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (compound_statement)
  (field_declaration_list)
  (enumerator_list)
  (initializer_list)
  (case_statement)
  (argument_list)
  (parameter_list)
  (layout_qualifiers)
] @indent

[
  "}"
  ")"
  "]"
] @outdent

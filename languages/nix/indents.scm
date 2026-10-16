; Nix: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. The words that
; end the bindings of a let go back to the level of the let.

[
  (attrset_expression)
  (rec_attrset_expression)
  (let_attrset_expression)
  (let_expression)
  (list_expression)
  (parenthesized_expression)
  (formals)
  (indented_string_expression)
] @indent

[
  "}"
  "]"
  ")"
  "in"
] @outdent

; SQL: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (column_definitions)
  (subquery)
  (cte)
  (list)
  (parenthesized_expression)
  (function_arguments)
  (case)
  (block)
] @indent

[
  ")"
  "]"
  (keyword_end)
] @outdent

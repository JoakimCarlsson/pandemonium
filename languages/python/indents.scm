; Python: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. A block is
; opened by the colon that ends its first line.

[
  (function_definition)
  (class_definition)
  (if_statement)
  (elif_clause)
  (else_clause)
  (for_statement)
  (while_statement)
  (try_statement)
  (except_clause)
  (finally_clause)
  (with_statement)
  (match_statement)
  (case_clause)
  (argument_list)
  (parameters)
  (list)
  (dictionary)
  (set)
  (tuple)
  (parenthesized_expression)
  (generator_expression)
  (list_comprehension)
  (dictionary_comprehension)
  (set_comprehension)
] @indent

[
  ")"
  "]"
  "}"
] @outdent

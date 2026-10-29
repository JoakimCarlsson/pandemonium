; Sample: a node captured @indent indents the lines inside it. An `if` is also
; marked @start.if, which the `valid_after` of `else` and `elif` in config.toml
; lines up with.

[
  (function_definition)
  (class_definition)
  (if_statement)
  (elif_clause)
  (else_clause)
  (for_statement)
  (while_statement)
  (argument_list)
  (parameters)
  (list)
  (dictionary)
] @indent

(if_statement) @start.if

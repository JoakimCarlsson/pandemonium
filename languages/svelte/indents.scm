; Svelte: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. An element is
; indented between its start and end tags, and a block between the tags that
; open and close it.

[
  (element)
  (script_element)
  (style_element)
  (if_statement)
  (else_if_block)
  (else_block)
  (each_statement)
  (await_statement)
  (then_block)
  (catch_block)
  (key_statement)
  (snippet_statement)
] @indent

[
  (end_tag)
  (if_end)
  (each_end)
  (await_end)
  (key_end)
  (snippet_end)
  (else_if_start)
  (else_start)
  (then_start)
  (catch_start)
] @outdent

; HTML: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. An element is
; indented between its start and end tags.

[
  (element)
  (script_element)
  (style_element)
] @indent

(end_tag) @outdent

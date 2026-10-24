; Markdown: a node captured @indent indents the lines inside it; a token
; captured @outdent closes one and goes back to the level it opened at. An item
; of a list goes on at the depth of its text.

(list_item
  (paragraph)) @indent

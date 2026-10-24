; Make: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. The lines of a
; rule's recipe are indented under it, and the words that end a conditional
; or a definition go back to the level of the word that opened it.

[
  (rule)
  (conditional)
  (define_directive)
] @indent

[
  "else"
  "endif"
  "endef"
] @outdent

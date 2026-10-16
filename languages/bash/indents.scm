; Bash: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. The words that
; end a block go back to the level of the word that opened it.

[
  (if_statement)
  (for_statement)
  (c_style_for_statement)
  (while_statement)
  (case_statement)
  (case_item)
  (compound_statement)
  (subshell)
  (command_substitution)
  (do_group)
  (array)
] @indent

[
  "fi"
  "done"
  "esac"
  "else"
  "elif"
  "}"
  ")"
] @outdent

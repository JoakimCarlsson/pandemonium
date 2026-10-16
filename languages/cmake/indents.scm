; CMake: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at. The commands
; that end a block go back to the level of the command that opened it.

[
  (function_def)
  (macro_def)
  (if_condition)
  (foreach_loop)
  (while_loop)
  (block_def)
  (argument_list)
] @indent

[
  (endfunction)
  (endmacro)
  (elseif)
  (else)
  (endif)
  (endforeach)
  (endwhile)
  (endblock)
  ")"
] @outdent

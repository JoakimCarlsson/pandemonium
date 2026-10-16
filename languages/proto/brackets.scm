; Protocol Buffers: tokens that pair up, @open with the @close of the same
; match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
(map_field
  "<" @open
  ">" @close)
(block_lit
  "<" @open
  ">" @close)

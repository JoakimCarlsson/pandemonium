; YAML: a node captured @indent indents the lines inside it; a token captured
; @outdent closes one and goes back to the level it opened at.

[
  (block_mapping_pair)
  (block_sequence_item)
  (flow_mapping)
  (flow_sequence)
] @indent

[
  "]"
  "}"
] @outdent

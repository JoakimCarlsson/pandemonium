; YAML: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

(block_mapping_pair
  key: (flow_node
    [
      (double_quote_scalar)
      (single_quote_scalar)
    ] @property))
(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @property)))
(flow_mapping
  (_
    key: (flow_node
      [
        (double_quote_scalar)
        (single_quote_scalar)
      ] @property)))
(flow_mapping
  (_
    key: (flow_node
      (plain_scalar
        (string_scalar) @property))))

(boolean_scalar) @constant.builtin
(null_scalar) @constant.builtin

[
  (integer_scalar)
  (float_scalar)
] @number

(timestamp_scalar) @constant

[
  (double_quote_scalar)
  (single_quote_scalar)
  (block_scalar)
  (string_scalar)
] @string
(escape_sequence) @string.escape

[
  (anchor_name)
  (alias_name)
] @label

(tag) @type

[
  (yaml_directive)
  (tag_directive)
  (reserved_directive)
] @keyword

[
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  "-"
  ":"
  ">"
  "?"
  "|"
] @punctuation.delimiter

[
  "*"
  "&"
  "---"
  "..."
] @punctuation.special

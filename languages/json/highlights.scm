; JSON: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(comment) @comment

(pair
  key: (string) @property)

(string) @string
(escape_sequence) @string.escape

(number) @number

[
  (null)
  (true)
  (false)
] @constant.builtin

[
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  ":"
] @punctuation.delimiter

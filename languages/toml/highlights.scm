; TOML: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(comment) @comment

(string) @string
(escape_sequence) @string.escape

[
  (integer)
  (float)
] @number

(boolean) @constant.builtin

[
  (offset_date_time)
  (local_date_time)
  (local_date)
  (local_time)
] @constant

(table
  (bare_key) @type)
(table
  (dotted_key
    (bare_key) @type))
(table
  (quoted_key) @type)
(table_array_element
  (bare_key) @type)
(table_array_element
  (dotted_key
    (bare_key) @type))

(pair
  (bare_key) @property)
(pair
  (dotted_key
    (bare_key) @property))
(pair
  (quoted_key) @property)
(inline_table
  (pair
    (bare_key) @property))

"=" @operator

[
  "."
  ","
] @punctuation.delimiter

[
  "["
  "]"
  "[["
  "]]"
  "{"
  "}"
] @punctuation.bracket

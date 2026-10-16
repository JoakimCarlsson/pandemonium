; CSS: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

[
  (comment)
  (js_comment)
] @comment

[
  "@media"
  "@import"
  "@charset"
  "@namespace"
  "@supports"
  "@keyframes"
  "@scope"
  (at_keyword)
  (to)
  (from)
  (important)
] @keyword

[
  "and"
  "or"
  "not"
  "only"
  "selector"
  "of"
] @keyword.operator

((property_name) @variable
  (#match? @variable "^--"))
((plain_value) @variable
  (#match? @variable "^--"))

(pseudo_element_selector
  (tag_name) @attribute)
(pseudo_class_selector
  (class_name) @attribute)

(tag_name) @tag
[
  (nesting_selector)
  (universal_selector)
] @tag

(class_name) @type
(id_name) @constant
(namespace_name) @type
(keyframes_name) @function

(attribute_name) @attribute
(attribute_selector
  (plain_value) @string)

[
  (property_name)
  (feature_name)
] @property

(function_name) @function

(string_value) @string
(escape_sequence) @string.escape

(color_value) @constant
(color_value
  "#" @constant)

[
  (integer_value)
  (float_value)
] @number
(unit) @number

[
  "~"
  ">"
  "+"
  "-"
  "*"
  "/"
  "="
  "^="
  "|="
  "~="
  "$="
  "*="
] @operator

[
  "#"
  ","
  "."
  ":"
  "::"
  ";"
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

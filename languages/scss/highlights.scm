; SCSS: what colour each node is. Where two patterns capture the same node, the
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
  "@at-root"
  "@debug"
  "@error"
  "@warn"
  "@extend"
  "@use"
  "@forward"
  "@include"
  "@mixin"
  "@function"
  "@return"
  "@if"
  "@else"
  "@each"
  "@for"
  "@while"
  (at_keyword)
  (to)
  (from)
  (important)
] @keyword

[
  "from"
  "through"
  "in"
  "if"
] @keyword

[
  "and"
  "or"
  "not"
  "only"
  "selector"
] @keyword.operator

(variable) @variable

((property_name) @variable
  (#match? @variable "^(--|\\$)"))
((plain_value) @variable
  (#match? @variable "^--"))

(mixin_statement
  name: (identifier) @function)
(function_statement
  name: (identifier) @function)
(include_statement
  (identifier) @function)
(function_name) @function

(placeholder) @type
(placeholder
  (identifier) @type)

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

(string_value) @string

(color_value) @constant
(color_value
  "#" @constant)

[
  (integer_value)
  (float_value)
] @number
(unit) @number

(interpolation
  [
    "#{"
    "}"
  ] @punctuation.special)

[
  "~"
  ">"
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "=="
  "!="
  "<"
  "<="
  ">="
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

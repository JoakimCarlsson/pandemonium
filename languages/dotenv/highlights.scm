; Dotenv: what colour each node is. A key is a property, a value is a string
; unless it is a number or a boolean, and ${NAME} inside a value names a variable.

(comment) @comment

(assignment
  key: (identifier) @property)

"export" @keyword

"=" @operator

[
  (boolean)
  (number)
] @constant

[
  (string)
  (string_content)
  (value)
] @string

(variable
  (identifier) @variable)

[
  "$"
  "{"
  "}"
] @punctuation.special

[
  "\""
  "'"
] @string

; Go: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

[
  (interpreted_string_literal)
  (raw_string_literal)
  (rune_literal)
] @string
(escape_sequence) @string.escape

[
  (int_literal)
  (float_literal)
  (imaginary_literal)
] @number

[
  (true)
  (false)
  (nil)
  (iota)
] @constant.builtin

(function_declaration
  name: (identifier) @function)
(method_declaration
  name: (field_identifier) @function.method)
(method_elem
  name: (field_identifier) @function.method)

(call_expression
  function: (identifier) @function.builtin
  (#match? @function.builtin "^(append|cap|clear|close|complex|copy|delete|imag|len|make|max|min|new|panic|print|println|real|recover)$"))
(call_expression
  function: (identifier) @function)
(call_expression
  function: (selector_expression
    field: (field_identifier) @function.method))

(package_clause
  (package_identifier) @namespace)
(package_identifier) @namespace
(import_spec
  name: (package_identifier) @namespace)

(type_identifier) @type
(type_spec
  name: (type_identifier) @type)

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

(field_identifier) @property

(parameter_declaration
  name: (identifier) @variable.parameter)
(variadic_parameter_declaration
  name: (identifier) @variable.parameter)

(label_name) @label

[
  "break"
  "case"
  "chan"
  "const"
  "continue"
  "default"
  "defer"
  "else"
  "fallthrough"
  "for"
  "func"
  "go"
  "goto"
  "if"
  "import"
  "interface"
  "map"
  "package"
  "range"
  "return"
  "select"
  "struct"
  "switch"
  "type"
  "var"
] @keyword

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "."
  ","
  ";"
  ":"
] @punctuation.delimiter

[
  "--"
  "-"
  "-="
  ":="
  "!"
  "!="
  "..."
  "*"
  "*="
  "/"
  "/="
  "&"
  "&&"
  "&="
  "&^"
  "&^="
  "%"
  "%="
  "^"
  "^="
  "+"
  "++"
  "+="
  "<-"
  "<"
  "<<"
  "<<="
  "<="
  "="
  "=="
  ">"
  ">="
  ">>"
  ">>="
  "|"
  "|="
  "||"
  "~"
] @operator

(identifier) @variable

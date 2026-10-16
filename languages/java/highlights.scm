; Java: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

[
  (line_comment)
  (block_comment)
] @comment

[
  (string_literal)
  (character_literal)
] @string
(escape_sequence) @string.escape
(string_interpolation
  [
    "\\{"
    "}"
  ] @punctuation.special)

[
  (hex_integer_literal)
  (decimal_integer_literal)
  (octal_integer_literal)
  (binary_integer_literal)
  (decimal_floating_point_literal)
  (hex_floating_point_literal)
] @number

[
  (true)
  (false)
  (null_literal)
] @constant.builtin

(this) @variable.builtin
(super) @variable.builtin

(annotation
  "@" @attribute
  name: (_) @attribute)
(marker_annotation
  "@" @attribute
  name: (_) @attribute)

(method_declaration
  name: (identifier) @function.method)
(annotation_type_element_declaration
  name: (identifier) @function.method)
(method_invocation
  name: (identifier) @function.method)
(method_reference
  "::"
  .
  (identifier) @function.method)

(interface_declaration
  name: (identifier) @type)
(annotation_type_declaration
  name: (identifier) @type)
(class_declaration
  name: (identifier) @type)
(record_declaration
  name: (identifier) @type)
(enum_declaration
  name: (identifier) @type)
(constructor_declaration
  name: (identifier) @constructor)
(compact_constructor_declaration
  name: (identifier) @constructor)

[
  (boolean_type)
  (integral_type)
  (floating_point_type)
  (void_type)
] @type.builtin
((type_identifier) @type.builtin
  (#eq? @type.builtin "var"))
(type_identifier) @type

((method_invocation
  object: (identifier) @type)
  (#match? @type "^[A-Z][a-z]"))
((method_reference
  .
  (identifier) @type)
  (#match? @type "^[A-Z][a-z]"))
((field_access
  object: (identifier) @type)
  (#match? @type "^[A-Z][a-z]"))
((scoped_identifier
  scope: (identifier) @type)
  (#match? @type "^[A-Z][a-z]"))
(import_declaration
  (scoped_identifier
    name: (identifier) @type)
  (#match? @type "^[A-Z][a-z]"))

(enum_constant
  name: (identifier) @constant)
((identifier) @constant
  (#match? @constant "^_*[A-Z][A-Z0-9_]+$"))

(field_declaration
  declarator: (variable_declarator
    name: (identifier) @property))
(field_access
  field: (identifier) @property)

(formal_parameter
  name: (identifier) @variable.parameter)
(spread_parameter
  (variable_declarator
    name: (identifier) @variable.parameter))
(catch_formal_parameter
  name: (identifier) @variable.parameter)
(inferred_parameters
  (identifier) @variable.parameter)
(lambda_expression
  parameters: (identifier) @variable.parameter)

(labeled_statement
  (identifier) @label)
(break_statement
  (identifier) @label)
(continue_statement
  (identifier) @label)

[
  "abstract"
  "assert"
  "break"
  "case"
  "catch"
  "class"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "exports"
  "extends"
  "final"
  "finally"
  "for"
  "if"
  "implements"
  "import"
  "instanceof"
  "interface"
  "@interface"
  "module"
  "native"
  "new"
  "non-sealed"
  "open"
  "opens"
  "package"
  "permits"
  "private"
  "protected"
  "provides"
  "public"
  "record"
  "requires"
  "return"
  "sealed"
  "static"
  "strictfp"
  "switch"
  "synchronized"
  "throw"
  "throws"
  "to"
  "transient"
  "transitive"
  "try"
  "uses"
  "volatile"
  "when"
  "while"
  "with"
  "yield"
] @keyword

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket
(type_arguments
  [
    "<"
    ">"
  ] @punctuation.bracket)
(type_parameters
  [
    "<"
    ">"
  ] @punctuation.bracket)

[
  ";"
  "."
  "..."
  ","
] @punctuation.delimiter

[
  "="
  "+"
  "-"
  "*"
  "/"
  "%"
  "++"
  "--"
  "&"
  "&&"
  "|"
  "||"
  "!"
  "!="
  "=="
  "<"
  "<="
  ">"
  ">="
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  ">>>="
  "->"
  "^"
  "~"
  "<<"
  ">>"
  ">>>"
  "::"
  "?"
  ":"
] @operator

(identifier) @variable

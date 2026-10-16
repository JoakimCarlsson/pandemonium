; GraphQL: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(comment) @comment

(description
  (string_value) @comment.documentation)

[
  "query"
  "mutation"
  "subscription"
  "fragment"
  "scalar"
  "input"
  "extend"
  "directive"
  "schema"
  "on"
  "repeatable"
  "implements"
  "enum"
  "union"
  "type"
  "interface"
] @keyword

(directive_location
  [
    (executable_directive_location)
    (type_system_directive_location)
  ] @type.builtin)

(directive_definition
  "@" @attribute
  (name) @attribute)
(directive
  "@" @attribute
  (name) @attribute)

(named_type
  (name) @type)
(scalar_type_definition
  (name) @type)
(object_type_definition
  (name) @type)
(interface_type_definition
  (name) @type)
(union_type_definition
  (name) @type)
(enum_type_definition
  (name) @type)
(input_object_type_definition
  (name) @type)
(scalar_type_extension
  (name) @type)
(object_type_extension
  (name) @type)
(interface_type_extension
  (name) @type)
(union_type_extension
  (name) @type)
(enum_type_extension
  (name) @type)
(input_object_type_extension
  (name) @type)

(operation_definition
  (name) @function)
(fragment_name
  (name) @function)

(variable) @variable

(argument
  (name) @variable.parameter)
(input_value_definition
  (name) @variable.parameter)

(field
  (alias
    (name) @property))
(field
  (name) @property)
(field_definition
  (name) @property)
(object_field
  (name) @property)

(enum_value_definition
  (enum_value
    (name) @constant))
(enum_value) @constant

(string_value) @string

[
  (int_value)
  (float_value)
] @number

[
  (boolean_value)
  (null_value)
] @constant.builtin

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ":"
  "|"
  "&"
  (comma)
] @punctuation.delimiter

[
  "..."
  "!"
  "="
] @operator

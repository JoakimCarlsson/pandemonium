; Protocol Buffers: what colour each node is. Where two patterns capture the
; same node, the one written first wins.

(comment) @comment

(import
  path: (string) @string.special)
(syntax
  version: (string) @string.special)
(edition
  year: (string) @string.special)
(string) @string
(reserved_identifier) @string
(escape_sequence) @string.escape

[
  (int_lit)
  (float_lit)
] @number

[
  (true)
  (false)
] @constant.builtin

(package
  (full_ident
    (identifier) @namespace))

[
  (message_name)
  (enum_name)
  (service_name)
] @type
(message_or_enum_type
  (identifier) @type)
(extend
  (full_ident
    (identifier) @type))
(extension_name
  type: (full_ident
    (identifier) @type))

(key_type) @type.builtin
(type) @type.builtin

(rpc_name) @function.method

(enum_field
  (identifier) @constant)
((full_ident
  (identifier) @constant)
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))
((identifier) @constant.builtin
  (#any-of? @constant.builtin "inf" "nan"))

(option
  (full_ident
    (identifier) @attribute))
(field_option
  (full_ident
    (identifier) @attribute))
(enum_value_option
  (full_ident
    (identifier) @attribute))
(extension_name
  name: (full_ident
    (identifier) @attribute))

(option
  (identifier) @property)
(field_option
  (identifier) @property)
(enum_value_option
  (identifier) @property)
(block_lit
  (identifier) @property)

(field
  (identifier) @property)
(map_field
  (identifier) @property)
(oneof_field
  (identifier) @property)
(oneof
  (identifier) @type)

[
  "edition"
  "enum"
  "export"
  "extend"
  "extensions"
  "group"
  "import"
  "local"
  "map"
  "max"
  "message"
  "oneof"
  "option"
  "optional"
  "package"
  "public"
  "repeated"
  "required"
  "reserved"
  "returns"
  "rpc"
  "service"
  "stream"
  "syntax"
  "to"
  "weak"
] @keyword

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "<"
  ">"
] @punctuation.bracket

[
  ";"
  ","
  "."
  ":"
] @punctuation.delimiter

[
  "="
  "-"
  "+"
] @operator

(identifier) @variable

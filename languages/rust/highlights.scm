; Rust: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(line_comment) @comment
(block_comment) @comment
(inner_doc_comment_marker) @comment
(outer_doc_comment_marker) @comment

(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(escape_sequence) @string.escape

(boolean_literal) @constant.builtin
(integer_literal) @number
(float_literal) @number

(attribute
  (identifier) @attribute)
(attribute
  (scoped_identifier
    name: (identifier) @attribute))
(attribute_item) @attribute
(inner_attribute_item) @attribute

(lifetime (identifier) @label)
(lifetime "'" @label)

(macro_invocation
  macro: (identifier) @function.macro
  "!" @function.macro)
(macro_invocation
  macro: (scoped_identifier
    name: (identifier) @function.macro)
  "!" @function.macro)
(macro_definition
  name: (identifier) @function.macro)

(function_item
  name: (identifier) @function)
(function_signature_item
  name: (identifier) @function)

(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))
(call_expression
  function: (scoped_identifier
    name: (identifier) @function))
(generic_function
  function: (identifier) @function)
(generic_function
  function: (scoped_identifier
    name: (identifier) @function))
(generic_function
  function: (field_expression
    field: (field_identifier) @function.method))

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

((identifier) @type
  (#match? @type "^[A-Z]"))

(type_identifier) @type
(primitive_type) @type.builtin

(scoped_identifier
  path: (identifier) @namespace)
(scoped_identifier
  path: (scoped_identifier
    name: (identifier) @namespace))
(scoped_type_identifier
  path: (identifier) @namespace)
(scoped_use_list
  path: (identifier) @namespace)
(use_declaration
  argument: (identifier) @namespace)
(mod_item
  name: (identifier) @namespace)

(field_identifier) @property
(shorthand_field_identifier) @property

(self) @variable.builtin
(crate) @keyword
(super) @keyword
(mutable_specifier) @keyword

(parameter
  pattern: (identifier) @variable.parameter)

[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "default"
  "dyn"
  "else"
  "enum"
  "extern"
  "fn"
  "for"
  "gen"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "macro_rules!"
  "match"
  "mod"
  "move"
  "pub"
  "raw"
  "ref"
  "return"
  "static"
  "struct"
  "trait"
  "type"
  "union"
  "unsafe"
  "use"
  "where"
  "while"
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
  "<" @punctuation.bracket
  ">" @punctuation.bracket)
(type_parameters
  "<" @punctuation.bracket
  ">" @punctuation.bracket)

[
  "::"
  ":"
  "."
  ","
  ";"
] @punctuation.delimiter

[
  "!"
  "!="
  "%"
  "%="
  "&"
  "&&"
  "&="
  "*"
  "*="
  "+"
  "+="
  "-"
  "-="
  "->"
  ".."
  "..="
  "/"
  "/="
  "<"
  "<<"
  "<<="
  "<="
  "="
  "=="
  "=>"
  ">"
  ">="
  ">>"
  ">>="
  "?"
  "^"
  "^="
  "|"
  "|="
  "||"
] @operator

(identifier) @variable

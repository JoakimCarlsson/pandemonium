; Swift: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

[
  (comment)
  (multiline_comment)
  (shebang_line)
] @comment

(line_string_literal
  [
    "\\("
    ")"
  ] @punctuation.special)
(multi_line_string_literal
  [
    "\\("
    ")"
  ] @punctuation.special)
(raw_str_interpolation
  [
    (raw_str_interpolation_start)
    ")"
  ] @punctuation.special)
[
  (line_str_text)
  (multi_line_str_text)
  (raw_str_part)
  (raw_str_end_part)
  (regex_literal)
  "\""
  "\"\"\""
] @string
(str_escaped_char) @string.escape

[
  (integer_literal)
  (hex_literal)
  (oct_literal)
  (bin_literal)
  (real_literal)
] @number

[
  (boolean_literal)
  "nil"
] @constant.builtin

[
  (self_expression)
  (super_expression)
] @variable.builtin
(wildcard_pattern) @variable.builtin

[
  (directive)
  (diagnostic)
  (availability_condition)
  (playground_literal)
  (key_path_string_expression)
  (selector_expression)
  (external_macro_definition)
] @function.macro
(macro_invocation
  "#" @function.macro
  (simple_identifier) @function.macro)
(special_literal) @constant.macro

(modifiers
  (attribute
    "@" @attribute
    (user_type
      (type_identifier) @attribute)))
(attribute
  "@" @attribute
  (user_type
    (type_identifier) @attribute))
[
  "@escaping"
  "@autoclosure"
] @attribute

(statement_label) @label

(import_declaration
  (identifier
    (simple_identifier) @namespace))

(function_declaration
  name: (simple_identifier) @function.method)
(protocol_function_declaration
  name: (simple_identifier) @function.method)
(macro_declaration
  (simple_identifier) @function.macro)
(init_declaration
  "init" @constructor)
(deinit_declaration
  "deinit" @constructor)

(call_expression
  .
  (simple_identifier) @variable
  (call_suffix
    (value_arguments
      .
      "[")))
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @property))
  (call_suffix
    (value_arguments
      .
      "[")))
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @function.method)))
(call_expression
  (prefix_expression
    (simple_identifier) @function.method))
((call_expression
  .
  (simple_identifier) @type)
  (#match? @type "^[A-Z]"))
(call_expression
  .
  (simple_identifier) @function)

((navigation_expression
  .
  (simple_identifier) @type)
  (#match? @type "^[A-Z]"))
(type_identifier) @type

(enum_entry
  name: (simple_identifier) @constant)
(prefix_expression
  "."
  (simple_identifier) @constant)

(parameter
  external_name: (simple_identifier) @variable.parameter)
(parameter
  name: (simple_identifier) @variable.parameter)
(lambda_parameter
  (simple_identifier) @variable.parameter)
(value_argument
  name: (value_argument_label
    (simple_identifier) @variable.parameter))

(class_body
  (property_declaration
    (pattern
      (simple_identifier) @property)))
(protocol_property_declaration
  (pattern
    (simple_identifier) @property))
(navigation_expression
  (navigation_suffix
    (simple_identifier) @property))

[
  (visibility_modifier)
  (member_modifier)
  (function_modifier)
  (property_modifier)
  (parameter_modifier)
  (inheritance_modifier)
  (mutation_modifier)
  (ownership_modifier)
  (property_behavior_modifier)
  (throws)
  (where_keyword)
  (getter_specifier)
  (setter_specifier)
  (modify_specifier)
  (else)
  (default_keyword)
  (try_operator)
  (throw_keyword)
  (catch_keyword)
  (as_operator)
] @keyword

[
  "import"
  "func"
  "let"
  "var"
  "class"
  "struct"
  "enum"
  "protocol"
  "extension"
  "actor"
  "typealias"
  "associatedtype"
  "init"
  "deinit"
  "subscript"
  "operator"
  "precedencegroup"
  "macro"
  "case"
  "if"
  "guard"
  "switch"
  "fallthrough"
  "for"
  "in"
  "while"
  "repeat"
  "do"
  "return"
  "break"
  "continue"
  "is"
  "some"
  "any"
  "async"
  "await"
  "inout"
  "indirect"
  "nonisolated"
  "override"
  "convenience"
  "required"
  "weak"
  "unowned"
  "didSet"
  "willSet"
  "consume"
  "borrowing"
  "consuming"
  "each"
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
  "."
  ";"
  ":"
  ","
] @punctuation.delimiter

(custom_operator) @operator
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "<"
  ">"
  "<<"
  ">>"
  "<="
  ">="
  "++"
  "--"
  "^"
  "&"
  "&&"
  "|"
  "||"
  "~"
  "!="
  "!=="
  "=="
  "==="
  "?"
  "??"
  "->"
  "..<"
  "..."
  (bang)
] @operator

(simple_identifier) @variable

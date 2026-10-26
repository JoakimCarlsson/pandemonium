; JavaScript: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(comment) @comment

[
  (string)
  (template_string)
] @string
(escape_sequence) @string.escape
(regex) @string.special
(number) @number

[
  (true)
  (false)
  (null)
  (undefined)
] @constant.builtin

(template_substitution
  "${" @punctuation.special
  "}" @punctuation.special)

(function_declaration
  name: (identifier) @function)
(function_expression
  name: (identifier) @function)
(generator_function_declaration
  name: (identifier) @function)
(method_definition
  name: (property_identifier) @function.method)
(pair
  key: (property_identifier) @function.method
  value: [(function_expression) (arrow_function)])
(variable_declarator
  name: (identifier) @function
  value: [(function_expression) (arrow_function)])
(assignment_expression
  left: (member_expression
    property: (property_identifier) @function.method)
  right: [(function_expression) (arrow_function)])

(call_expression
  function: (identifier) @function)
(call_expression
  function: (member_expression
    property: (property_identifier) @function.method))
(new_expression
  constructor: (identifier) @type)

(jsx_opening_element
  name: (identifier) @type
  (#match? @type "^[A-Z]"))
(jsx_closing_element
  name: (identifier) @type
  (#match? @type "^[A-Z]"))
(jsx_self_closing_element
  name: (identifier) @type
  (#match? @type "^[A-Z]"))
(jsx_opening_element
  name: (identifier) @tag)
(jsx_closing_element
  name: (identifier) @tag)
(jsx_self_closing_element
  name: (identifier) @tag)
(jsx_attribute
  (property_identifier) @attribute)
(jsx_opening_element
  [
    "<"
    ">"
  ] @punctuation.bracket)
(jsx_closing_element
  [
    "</"
    ">"
  ] @punctuation.bracket)
(jsx_self_closing_element
  [
    "<"
    "/>"
  ] @punctuation.bracket)
(jsx_opening_element
  name: (member_expression
    object: (identifier) @type
    property: (property_identifier) @type))
(jsx_closing_element
  name: (member_expression
    object: (identifier) @type
    property: (property_identifier) @type))
(jsx_self_closing_element
  name: (member_expression
    object: (identifier) @type
    property: (property_identifier) @type))
(jsx_namespace_name
  (identifier) @tag)
(jsx_attribute
  (jsx_namespace_name
    (identifier) @attribute))
(jsx_expression
  [
    "{"
    "}"
  ] @punctuation.special)
(html_character_reference) @string.escape

(this) @variable.builtin
(super) @variable.builtin

([
  (identifier)
  (shorthand_property_identifier)
  (shorthand_property_identifier_pattern)
] @constant
  (#match? @constant "^[A-Z_][A-Z0-9_]+$"))

((identifier) @type
  (#match? @type "^[A-Z]"))

(property_identifier) @property
(shorthand_property_identifier) @property

[
  "as"
  "async"
  "await"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "default"
  "delete"
  "do"
  "else"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "function"
  "get"
  "if"
  "import"
  "in"
  "instanceof"
  "let"
  "new"
  "of"
  "return"
  "set"
  "static"
  "switch"
  "target"
  "throw"
  "try"
  "typeof"
  "var"
  "void"
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

[
  ";"
  (optional_chain)
  "."
  ","
  ":"
] @punctuation.delimiter

[
  "-"
  "--"
  "-="
  "+"
  "++"
  "+="
  "*"
  "*="
  "**"
  "**="
  "/"
  "/="
  "%"
  "%="
  "<"
  "<="
  "<<"
  "<<="
  "="
  "=="
  "==="
  "!"
  "!="
  "!=="
  "=>"
  ">"
  ">="
  ">>"
  ">>="
  ">>>"
  ">>>="
  "~"
  "^"
  "&"
  "|"
  "^="
  "&="
  "|="
  "&&"
  "||"
  "??"
  "&&="
  "||="
  "??="
  "?"
  "..."
] @operator

(identifier) @variable

; C#: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

(interpolation
  (interpolation_brace) @punctuation.special)
[
  (string_literal)
  (verbatim_string_literal)
  (raw_string_literal)
  (character_literal)
  (interpolated_string_expression)
  (interpolation_start)
  (interpolation_quote)
] @string
(escape_sequence) @string.escape

[
  (integer_literal)
  (real_literal)
] @number

[
  (boolean_literal)
  (null_literal)
] @constant.builtin

[
  "this"
  "base"
] @variable.builtin
(discard) @variable.builtin

[
  "#define"
  "#undef"
  "#if"
  "#elif"
  "#else"
  "#endif"
  "#region"
  "#endregion"
  "#error"
  "#warning"
  "#line"
  "#nullable"
  "#pragma"
] @keyword
(preproc_arg) @constant.macro
(preproc_define
  (preproc_arg) @constant.macro)

(attribute
  name: (identifier) @attribute)
(attribute
  name: (qualified_name
    name: (identifier) @attribute))

(namespace_declaration
  name: (identifier) @namespace)
(file_scoped_namespace_declaration
  name: (identifier) @namespace)
(using_directive
  (identifier) @namespace)
(qualified_name
  (identifier) @namespace)

(method_declaration
  name: (identifier) @function.method)
(local_function_statement
  name: (identifier) @function)
(invocation_expression
  function: (member_access_expression
    name: [
      (identifier) @function.method
      (generic_name
        (identifier) @function.method)
    ]))
(invocation_expression
  function: [
    (identifier) @function
    (generic_name
      (identifier) @function)
  ])

(constructor_declaration
  name: (identifier) @constructor)
(destructor_declaration
  name: (identifier) @constructor)

(interface_declaration
  name: (identifier) @type)
(class_declaration
  name: (identifier) @type)
(struct_declaration
  name: (identifier) @type)
(record_declaration
  name: (identifier) @type)
(enum_declaration
  name: (identifier) @type)
(delegate_declaration
  name: (identifier) @type)
(type_parameter
  (identifier) @type)
(type_parameter_constraints_clause
  (identifier) @type)
(generic_name
  (identifier) @type)
(type_argument_list
  (identifier) @type)
(base_list
  (identifier) @type)
(object_creation_expression
  type: (identifier) @type)
(as_expression
  right: (identifier) @type)
(is_expression
  right: (identifier) @type)
(typeof_expression
  type: (identifier) @type)
(_
  type: (identifier) @type)
(array_type
  type: (identifier) @type)
(nullable_type
  type: (identifier) @type)
(predefined_type) @type.builtin
(implicit_type) @type.builtin

(enum_member_declaration
  name: (identifier) @constant)
((identifier) @constant
  (#match? @constant "^_*[A-Z][A-Z0-9_]+$"))

(property_declaration
  name: (identifier) @property)
(event_declaration
  name: (identifier) @property)
(event_field_declaration
  (variable_declaration
    (variable_declarator
      name: (identifier) @property)))
(field_declaration
  (variable_declaration
    (variable_declarator
      name: (identifier) @property)))
(member_access_expression
  name: (identifier) @property)
(initializer_expression
  (assignment_expression
    left: (identifier) @property))

(parameter
  name: (identifier) @variable.parameter)
(implicit_parameter) @variable.parameter

(labeled_statement
  (identifier) @label)
(goto_statement
  (identifier) @label)

(modifier) @keyword
(accessor_declaration
  name: _ @keyword)

[
  "add"
  "alias"
  "and"
  "as"
  "ascending"
  "await"
  "break"
  "by"
  "case"
  "catch"
  "checked"
  "class"
  "continue"
  "default"
  "delegate"
  "descending"
  "do"
  "else"
  "enum"
  "equals"
  "event"
  "explicit"
  "extern"
  "finally"
  "fixed"
  "for"
  "foreach"
  "from"
  "get"
  "global"
  "goto"
  "group"
  "if"
  "implicit"
  "in"
  "init"
  "interface"
  "into"
  "is"
  "join"
  "let"
  "lock"
  "namespace"
  "new"
  "not"
  "notnull"
  "on"
  "operator"
  "or"
  "orderby"
  "out"
  "params"
  "record"
  "ref"
  "remove"
  "return"
  "scoped"
  "select"
  "set"
  "sizeof"
  "stackalloc"
  "struct"
  "switch"
  "throw"
  "try"
  "typeof"
  "unchecked"
  "unmanaged"
  "using"
  "when"
  "where"
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
(type_argument_list
  [
    "<"
    ">"
  ] @punctuation.bracket)
(type_parameter_list
  [
    "<"
    ">"
  ] @punctuation.bracket)

[
  ";"
  "."
  ","
  "::"
] @punctuation.delimiter

[
  "--"
  "-"
  "-="
  "&"
  "&="
  "&&"
  "+"
  "++"
  "+="
  "<"
  "<="
  "<<"
  "<<="
  "="
  "=="
  "!"
  "!="
  "=>"
  "->"
  ">"
  ">="
  ">>"
  ">>="
  ">>>"
  ">>>="
  "|"
  "|="
  "||"
  "?"
  "??"
  "??="
  "^"
  "^="
  "~"
  "*"
  "*="
  "/"
  "/="
  "%"
  "%="
  ":"
  ".."
] @operator

(identifier) @variable

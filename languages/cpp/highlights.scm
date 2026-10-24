; C++: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

[
  (string_literal)
  (raw_string_literal)
  (system_lib_string)
  (char_literal)
] @string
(escape_sequence) @string.escape

(number_literal) @number
(user_defined_literal
  (literal_suffix) @operator)

[
  (true)
  (false)
  (null)
] @constant.builtin
(this) @variable.builtin

((identifier) @constant.builtin
  (#any-of? @constant.builtin "stdin" "stdout" "stderr" "__FILE__" "__LINE__" "__DATE__" "__TIME__" "__func__" "__FUNCTION__" "__PRETTY_FUNCTION__" "__VA_ARGS__" "__STDC__" "__STDC_VERSION__" "__cplusplus"))

(preproc_def
  name: (identifier) @constant.macro)
(preproc_function_def
  name: (identifier) @function.macro)
(preproc_ifdef
  name: (identifier) @constant.macro)
(preproc_defined
  (identifier) @constant.macro)
(preproc_defined) @function.macro
(preproc_params
  (identifier) @variable.parameter)

(function_declarator
  declarator: (identifier) @function)
(function_declarator
  declarator: (field_identifier) @function.method)
(function_declarator
  declarator: (qualified_identifier
    name: (identifier) @function))
(function_declarator
  declarator: (qualified_identifier
    name: (qualified_identifier
      name: (identifier) @function)))
(function_declarator
  declarator: (qualified_identifier
    name: (destructor_name
      (identifier) @function.method)))
(function_declarator
  declarator: (template_function
    name: (identifier) @function))
(function_declarator
  declarator: (template_method
    name: (field_identifier) @function.method))
(function_declarator
  declarator: (parenthesized_declarator
    (pointer_declarator
      declarator: (field_identifier) @function)))
(destructor_name
  (identifier) @function.method)
(operator_name) @function
"operator" @function
"static_assert" @function.builtin

((call_expression
  function: (identifier) @function.builtin)
  (#match? @function.builtin "^__builtin_"))
((call_expression
  function: (identifier) @constructor)
  (#match? @constructor "^[A-Z]"))
((call_expression
  function: (qualified_identifier
    name: (identifier) @constructor))
  (#match? @constructor "^[A-Z]"))
((field_initializer
  (field_identifier) @constructor)
  (#match? @constructor "^[A-Z]"))
(call_expression
  function: (identifier) @function)
(call_expression
  function: (qualified_identifier
    name: (identifier) @function))
(call_expression
  function: (qualified_identifier
    name: (qualified_identifier
      name: (identifier) @function)))
(call_expression
  function: (template_function
    name: (identifier) @function))
(call_expression
  function: (qualified_identifier
    name: (template_function
      name: (identifier) @function)))
(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))
(call_expression
  function: (field_expression
    field: (template_method
      name: (field_identifier) @function.method)))

(enumerator
  name: (identifier) @constant)
(case_statement
  value: (identifier) @constant)
(case_statement
  value: (qualified_identifier
    name: (identifier) @constant))
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

(namespace_identifier) @namespace
(using_declaration
  "namespace"
  (identifier) @namespace)

[
  (primitive_type)
  (sized_type_specifier)
  (auto)
] @type.builtin
(type_identifier) @type
(type_descriptor) @type
(concept_definition
  name: (identifier) @type)
(template_function
  name: (identifier) @function)

(field_identifier) @property
(field_designator) @property

(statement_identifier) @label

(parameter_declaration
  declarator: (identifier) @variable.parameter)
(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @variable.parameter))
(parameter_declaration
  declarator: (reference_declarator
    (identifier) @variable.parameter))
(parameter_declaration
  declarator: (array_declarator
    declarator: (identifier) @variable.parameter))
(optional_parameter_declaration
  declarator: (identifier) @variable.parameter)
(optional_parameter_declaration
  declarator: (reference_declarator
    (identifier) @variable.parameter))
(variadic_parameter_declaration
  declarator: (variadic_declarator
    (identifier) @variable.parameter))

(attribute_declaration
  (attribute
    name: (identifier) @attribute))
(attribute_declaration
  (attribute
    prefix: (identifier) @attribute))
[
  (ms_pointer_modifier)
  "__attribute__"
  "__declspec"
] @attribute

[
  (storage_class_specifier)
  (type_qualifier)
  (virtual_specifier)
  (access_specifier)
] @keyword

[
  "#define"
  "#elif"
  "#elifdef"
  "#elifndef"
  "#else"
  "#endif"
  "#if"
  "#ifdef"
  "#ifndef"
  "#include"
  (preproc_directive)
] @keyword

[
  "and"
  "and_eq"
  "asm"
  "__asm__"
  "bitand"
  "bitor"
  "break"
  "case"
  "catch"
  "class"
  "co_await"
  "co_return"
  "co_yield"
  "compl"
  "concept"
  "consteval"
  "constexpr"
  "constinit"
  "continue"
  "decltype"
  "default"
  "delete"
  "do"
  "else"
  "enum"
  "explicit"
  "extern"
  "final"
  "for"
  "friend"
  "goto"
  "if"
  "inline"
  "mutable"
  "namespace"
  "new"
  "noexcept"
  "not"
  "not_eq"
  "offsetof"
  "or"
  "or_eq"
  "override"
  "private"
  "protected"
  "public"
  "requires"
  "return"
  "sizeof"
  "alignof"
  "struct"
  "switch"
  "template"
  "throw"
  "try"
  "typedef"
  "typename"
  "union"
  "using"
  "virtual"
  "while"
  "xor"
  "xor_eq"
] @keyword

(template_argument_list
  [
    "<"
    ">"
  ] @punctuation.bracket)
(template_parameter_list
  [
    "<"
    ">"
  ] @punctuation.bracket)

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
  "::"
  "..."
] @punctuation.delimiter

[
  "="
  "-"
  "*"
  "/"
  "+"
  "%"
  "~"
  "|"
  "&"
  "^"
  "<<"
  ">>"
  "->"
  "->*"
  ".*"
  "<"
  "<="
  ">="
  ">"
  "=="
  "!="
  "<=>"
  "!"
  "&&"
  "||"
  "-="
  "+="
  "*="
  "/="
  "%="
  "|="
  "&="
  "^="
  ">>="
  "<<="
  "--"
  "++"
  "?"
] @operator

(identifier) @variable

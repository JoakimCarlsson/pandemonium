; C: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

[
  (string_literal)
  (system_lib_string)
  (char_literal)
] @string
(escape_sequence) @string.escape

(number_literal) @number

[
  (true)
  (false)
  (null)
] @constant.builtin

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
  declarator: (parenthesized_declarator
    (pointer_declarator
      declarator: (field_identifier) @function)))

((call_expression
  function: (identifier) @function.builtin)
  (#match? @function.builtin "^__builtin_"))
(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))

(enumerator
  name: (identifier) @constant)
(case_statement
  value: (identifier) @constant)
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

(primitive_type) @type.builtin
(sized_type_specifier) @type.builtin
(type_identifier) @type
(type_descriptor) @type

(field_identifier) @property
(field_designator) @property

(statement_identifier) @label

(parameter_declaration
  declarator: (identifier) @variable.parameter)
(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @variable.parameter))
(parameter_declaration
  declarator: (array_declarator
    declarator: (identifier) @variable.parameter))

[
  (attribute_declaration)
  (ms_pointer_modifier)
  "__attribute__"
  "__declspec"
] @attribute

[
  (storage_class_specifier)
  (type_qualifier)
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
  "asm"
  "__asm__"
  "break"
  "case"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extern"
  "for"
  "goto"
  "if"
  "return"
  "sizeof"
  "offsetof"
  "_Alignof"
  "alignof"
  "_Generic"
  "struct"
  "switch"
  "typedef"
  "union"
  "while"
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
  "<"
  "<="
  ">="
  ">"
  "=="
  "!="
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

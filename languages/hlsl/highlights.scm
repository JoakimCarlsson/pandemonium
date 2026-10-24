; HLSL: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

(string_literal) @string
(system_lib_string) @string
(escape_sequence) @string.escape

(number_literal) @number
[
  (true)
  (false)
] @constant.builtin

(preproc_def
  name: (identifier) @constant)
(preproc_function_def
  name: (identifier) @function)

(hlsl_attribute
  [
    "["
    "]"
  ] @attribute)
(hlsl_attribute
  (identifier) @attribute)
(hlsl_attribute
  (call_expression
    function: (identifier) @attribute))

(semantics
  (call_expression
    function: (identifier) @keyword
    (#any-of? @keyword "register" "packoffset")))
(semantics
  (identifier) @attribute)
(field_declaration
  (bitfield_clause
    (identifier) @attribute))

((type_identifier) @keyword
  (#any-of? @keyword "cbuffer" "tbuffer"))

(function_declarator
  declarator: (identifier) @function)
(function_declarator
  declarator: (qualified_identifier
    name: (identifier) @function))

(call_expression
  function: (identifier) @type
  (#match? @type "^(bool|int|uint|dword|half|float|double|min16float|min10float|min16int|min12int|min16uint|u?int(16|64)_t|float(16|64)_t)([1-4](x[1-4])?)?$"))
(call_expression
  function: (identifier) @function)
(call_expression
  function: (qualified_identifier
    name: (identifier) @function))
(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))

(namespace_identifier) @namespace

[
  (primitive_type)
  (type_identifier)
  (sized_type_specifier)
  (auto)
] @type

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

(field_identifier) @property

(parameter_declaration
  declarator: (identifier) @variable.parameter)

(statement_identifier) @label

(this) @variable.builtin

[
  "break"
  "case"
  "cbuffer"
  "class"
  "const"
  "continue"
  "default"
  "discard"
  "do"
  "else"
  "enum"
  "export"
  "extern"
  "for"
  "if"
  "in"
  "inline"
  "inout"
  "namespace"
  "out"
  "return"
  "static"
  "struct"
  "switch"
  "template"
  "typedef"
  "typename"
  "union"
  "using"
  "volatile"
  "while"
  (qualifiers)
  (type_qualifier)
  (storage_class_specifier)
] @keyword

[
  "#define"
  "#elif"
  "#else"
  "#endif"
  "#if"
  "#ifdef"
  "#ifndef"
  "#include"
  (preproc_directive)
] @keyword

(template_argument_list
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
] @punctuation.delimiter

[
  "-"
  "--"
  "-="
  "!"
  "!="
  "*"
  "*="
  "/"
  "/="
  "&"
  "&&"
  "&="
  "%"
  "%="
  "^"
  "^="
  "+"
  "++"
  "+="
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
  "?"
] @operator

(identifier) @variable

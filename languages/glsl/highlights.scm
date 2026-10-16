; GLSL: what colour each node is. Where two patterns capture the same node, the
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
(preproc_extension
  extension: (identifier) @constant)
(extension_behavior) @keyword

(function_declarator
  declarator: (identifier) @function)

(call_expression
  function: (identifier) @type
  (#match? @type "^([A-Z]|([biud]?vec[234]|d?mat[234](x[234])?|bool|int|uint|float|double)$)"))
(call_expression
  function: (identifier) @function)

(layout_qualifiers
  (qualifier
    (identifier) @attribute))

(declaration
  (identifier) @type
  .
  (field_declaration_list))

[
  (primitive_type)
  (type_identifier)
  (sized_type_specifier)
] @type

((identifier) @variable.builtin
  (#match? @variable.builtin "^gl_"))
((identifier) @keyword
  (#eq? @keyword "discard"))

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

(field_identifier) @property

(parameter_declaration
  declarator: (identifier) @variable.parameter)

(statement_identifier) @label

[
  "break"
  "case"
  "const"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extern"
  "for"
  "if"
  "inline"
  "return"
  "sizeof"
  "static"
  "struct"
  "switch"
  "typedef"
  "union"
  "volatile"
  "while"
  "in"
  "out"
  "inout"
  "uniform"
  "shared"
  "layout"
  "attribute"
  "varying"
  "buffer"
  "coherent"
  "readonly"
  "writeonly"
  "restrict"
  "precision"
  "highp"
  "mediump"
  "lowp"
  "centroid"
  "sample"
  "patch"
  "smooth"
  "flat"
  "noperspective"
  "invariant"
  "precise"
  "subroutine"
  (type_qualifier)
  (storage_class_specifier)
  (extension_storage_class)
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

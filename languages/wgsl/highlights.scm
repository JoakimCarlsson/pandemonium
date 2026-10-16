; WGSL: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

[
  (line_comment)
  (block_comment)
] @comment

[
  (int_literal)
  (float_literal)
] @number
(bool_literal) @constant.builtin

(attribute
  "@" @attribute
  name: (identifier) @attribute)

(import_item
  name: (identifier) @type
  (#match? @type "^[A-Z]"))
(import_item
  rename: (identifier) @type
  (#match? @type "^[A-Z]"))
(import_path
  (identifier) @namespace)
(ident_path
  (identifier) @namespace)

(function_header
  name: (identifier) @function)

(call_expression
  (identifier) @type
  (#match? @type "^([A-Z]|(bool|[iuf]32|f16|vec[234][iufh]?|mat[234]x[234][fh]?|array|atomic|ptr)$)"))
(call_expression
  (identifier) @function)
(func_call_statement
  (identifier) @function)

(struct_decl
  name: (identifier) @type)
(type_alias_decl
  name: (identifier) @type)
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

(type_specifier
  (identifier) @type)
(type_specifier
  (template_list
    (identifier) @type))
(template_list
  (template_list
    (identifier) @type))

(variable_decl
  (template_list
    (identifier) @keyword))

(struct_member
  name: (identifier) @property)
(named_component_expression
  component: (_) @property)

(param
  name: (identifier) @variable.parameter)

[
  "alias"
  "as"
  "break"
  "case"
  "const"
  "const_assert"
  "continue"
  "continuing"
  "default"
  "diagnostic"
  "discard"
  "else"
  "enable"
  "fn"
  "for"
  "if"
  "import"
  "let"
  "loop"
  "override"
  "private"
  "public"
  "requires"
  "return"
  "struct"
  "switch"
  "var"
  "virtual"
  "while"
  "#import"
  (preproc_directive)
] @keyword

[
  (template_args_start)
  (template_args_end)
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
  "->"
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
  "="
  "=="
  "|"
  "|="
  "||"
  "~"
  (less_than)
  (less_than_equal)
  (greater_than)
  (greater_than_equal)
  (shift_left)
  (shift_left_assign)
  (shift_right)
  (shift_right_assign)
] @operator

(identifier) @variable

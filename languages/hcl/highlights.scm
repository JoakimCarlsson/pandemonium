; HCL: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

[
  (quoted_template_start)
  (quoted_template_end)
  (heredoc_identifier)
] @string
(string_lit
  (template_literal) @string)
(quoted_template
  (template_literal) @string)
(template_for
  (template_literal) @string)
(template_if
  (template_literal) @string)
((heredoc_template
  (template_literal) @string
  (heredoc_identifier) @_tag)
  (#not-any-of? @_tag "JSON" "json" "YAML" "yaml" "YML" "yml" "BASH" "bash" "SH" "sh" "SHELL" "shell" "PYTHON" "python" "PY" "py" "SQL" "sql"))

[
  (template_interpolation_start)
  (template_interpolation_end)
  (template_directive_start)
  (template_directive_end)
  (strip_marker)
  (heredoc_start)
] @punctuation.special

(numeric_lit) @number

[
  (bool_lit)
  (null_lit)
] @constant.builtin

(config_file
  (body
    (block
      .
      (identifier) @keyword)))
(block
  .
  (identifier) @type)
(block
  (identifier) @type)

(function_call
  (identifier) @function)

(attribute
  (identifier) @property)
(object_elem
  key: (expression
    (variable_expr
      (identifier) @property)))
(get_attr
  (identifier) @property)

[
  "for"
  "endfor"
  "in"
  "if"
  "else"
  "endif"
] @keyword

[
  "!"
  "!="
  "%"
  "&&"
  "*"
  "+"
  "-"
  "/"
  "<"
  "<="
  "="
  "=="
  "=>"
  ">"
  ">="
  "?"
  "||"
  (ellipsis)
] @operator

[
  (block_start)
  (block_end)
  (object_start)
  (object_end)
  (tuple_start)
  (tuple_end)
  "("
  ")"
  "["
  "]"
  "[*]"
] @punctuation.bracket

[
  "."
  ".*"
  ","
  ":"
] @punctuation.delimiter

(identifier) @variable

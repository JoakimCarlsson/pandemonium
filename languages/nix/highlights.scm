; Nix: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

(interpolation
  "${" @punctuation.special
  "}" @punctuation.special)

[
  (escape_sequence)
  (dollar_escape)
] @string.escape
(string_expression) @string
(indented_string_expression) @string

[
  (path_expression)
  (hpath_expression)
  (spath_expression)
] @string.special.path
(uri_expression) @markup.link.url

(integer_expression) @number
(float_expression) @number

(variable_expression
  name: (identifier) @constant.builtin
  (#any-of? @constant.builtin "true" "false" "null"))
(variable_expression
  name: (identifier) @keyword
  (#any-of? @keyword "import" "abort" "throw"))
(variable_expression
  name: (identifier) @namespace
  (#eq? @namespace "builtins"))

(select_expression
  expression: (variable_expression
    name: (identifier) @_builtins)
  attrpath: (attrpath
    attr: (identifier) @function.builtin)
  (#eq? @_builtins "builtins"))

(binding
  attrpath: (attrpath
    attr: (identifier) @function .)
  expression: (function_expression))
(apply_expression
  function: (variable_expression
    name: (identifier) @function))
(apply_expression
  function: (select_expression
    attrpath: (attrpath
      attr: (identifier) @function .)))

(formal
  name: (identifier) @variable.parameter)
(function_expression
  universal: (identifier) @variable.parameter)
(ellipses) @variable.parameter

(binding
  attrpath: (attrpath
    attr: (identifier) @property))
(inherited_attrs
  attr: (identifier) @property)
(select_expression
  attrpath: (attrpath
    attr: (identifier) @property))
(has_attr_expression
  attrpath: (attrpath
    attr: (identifier) @property))

(variable_expression) @variable

[
  "assert"
  "else"
  "if"
  "in"
  "inherit"
  "let"
  "or"
  "rec"
  "then"
  "with"
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
  ";"
  ":"
  ","
] @punctuation.delimiter

(unary_expression
  operator: _ @operator)
(binary_expression
  operator: _ @operator)
(has_attr_expression
  operator: _ @operator)

[
  "="
  "@"
  "?"
] @operator

; Zig: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

((comment) @comment.documentation
  (#match? @comment.documentation "^//[!/]"))
(comment) @comment

[
  (string)
  (multiline_string)
] @string
(character) @character
(escape_sequence) @string.escape

(integer) @number
(float) @number

(boolean) @constant.builtin
[
  "null"
  "undefined"
  "unreachable"
] @constant.builtin

(builtin_identifier) @function.builtin

(variable_declaration
  (identifier) @namespace
  (builtin_function
    (builtin_identifier) @_import)
  (#any-of? @_import "@import" "@cImport"))

(variable_declaration
  (identifier) @type
  [
    (struct_declaration)
    (enum_declaration)
    (union_declaration)
    (opaque_declaration)
    (error_set_declaration)
  ])

(function_declaration
  name: (identifier) @function)
(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    member: (identifier) @function.method))

[
  (builtin_type)
  "anyframe"
] @type.builtin

(error_set_declaration
  (identifier) @constant)
(enum_declaration
  (container_field
    type: (identifier) @constant))

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))
((identifier) @type
  (#match? @type "^[A-Z][a-zA-Z0-9_]*$"))
(parameter
  type: (identifier) @type)

(block_label
  (identifier) @label)
(break_label
  (identifier) @label)

(container_field
  name: (identifier) @property)
(field_initializer
  .
  (identifier) @property)
(field_expression
  member: (identifier) @property)

(parameter
  name: (identifier) @variable.parameter)

((identifier) @variable.builtin
  (#eq? @variable.builtin "_"))
(calling_convention
  (identifier) @variable.builtin)

[
  "addrspace"
  "align"
  "allowzero"
  "and"
  "asm"
  "async"
  "await"
  "break"
  "callconv"
  "catch"
  "comptime"
  "const"
  "continue"
  "defer"
  "else"
  "enum"
  "errdefer"
  "error"
  "export"
  "extern"
  "fn"
  "for"
  "if"
  "inline"
  "linksection"
  "noalias"
  "noinline"
  "nosuspend"
  "opaque"
  "or"
  "orelse"
  "packed"
  "pub"
  "resume"
  "return"
  "struct"
  "suspend"
  "switch"
  "test"
  "threadlocal"
  "try"
  "union"
  "usingnamespace"
  "var"
  "volatile"
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
(payload
  "|" @punctuation.bracket)

[
  ";"
  "."
  ","
  ":"
  "=>"
  "->"
] @punctuation.delimiter

[
  "="
  "*="
  "*%="
  "*|="
  "/="
  "%="
  "+="
  "+%="
  "+|="
  "-="
  "-%="
  "-|="
  "<<="
  "<<|="
  ">>="
  "&="
  "^="
  "|="
  "!"
  "~"
  "-"
  "-%"
  "&"
  "=="
  "!="
  ">"
  ">="
  "<="
  "<"
  "^"
  "|"
  "<<"
  ">>"
  "<<|"
  "+"
  "++"
  "+%"
  "+|"
  "-|"
  "*"
  "/"
  "%"
  "**"
  "*%"
  "*|"
  "||"
  ".*"
  ".?"
  "?"
  ".."
  "..."
] @operator

(identifier) @variable

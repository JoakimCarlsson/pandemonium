; Lua: what colour each node is. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment
(hash_bang_line) @comment

(string) @string
(escape_sequence) @string.escape

(number) @number

[
  (nil)
  (true)
  (false)
] @constant.builtin
(vararg_expression) @constant.builtin

((identifier) @variable.builtin
  (#eq? @variable.builtin "self"))
((identifier) @constant.builtin
  (#eq? @constant.builtin "_VERSION"))
((identifier) @namespace
  (#any-of? @namespace "_G" "_ENV" "coroutine" "debug" "io" "math" "os" "package" "string" "table" "utf8" "bit32" "jit" "ffi"))

(function_declaration
  name: (identifier) @function)
(function_declaration
  name: (dot_index_expression
    field: (identifier) @function))
(function_declaration
  name: (method_index_expression
    method: (identifier) @function.method))

(assignment_statement
  (variable_list
    .
    name: [
      (identifier) @function
      (dot_index_expression
        field: (identifier) @function)
    ])
  (expression_list
    .
    value: (function_definition)))
(field
  name: (identifier) @function
  value: (function_definition))

((function_call
  name: (identifier) @function.builtin)
  (#any-of? @function.builtin "assert" "collectgarbage" "dofile" "error" "getfenv" "getmetatable" "ipairs" "load" "loadfile" "loadstring" "module" "next" "pairs" "pcall" "print" "rawequal" "rawget" "rawlen" "rawset" "require" "select" "setfenv" "setmetatable" "tonumber" "tostring" "type" "unpack" "xpcall"))
(function_call
  name: (identifier) @function)
(function_call
  name: (dot_index_expression
    field: (identifier) @function))
(function_call
  name: (method_index_expression
    method: (identifier) @function.method))

(label_statement
  (identifier) @label)
(goto_statement
  (identifier) @label)

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))

(parameters
  name: (identifier) @variable.parameter)

(field
  name: (identifier) @property)
(dot_index_expression
  field: (identifier) @property)

(attribute
  "<" @punctuation.bracket
  (identifier) @attribute
  ">" @punctuation.bracket)

[
  "and"
  "do"
  "else"
  "elseif"
  "end"
  "for"
  "function"
  "global"
  "goto"
  "if"
  "in"
  "local"
  "not"
  "or"
  "repeat"
  "return"
  "then"
  "until"
  "while"
] @keyword
(break_statement) @keyword

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
  ":"
  "::"
  ","
  "."
] @punctuation.delimiter

[
  "+"
  "-"
  "*"
  "/"
  "//"
  "%"
  "^"
  "#"
  "&"
  "~"
  "|"
  "<<"
  ">>"
  "=="
  "~="
  "<"
  "<="
  ">"
  ">="
  "="
  ".."
] @operator

(identifier) @variable

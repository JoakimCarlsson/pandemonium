; Python: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

(comment) @comment

(string) @string
(escape_sequence) @string.escape
(interpolation
  "{" @punctuation.special
  "}" @punctuation.special)

[
  (integer)
  (float)
] @number

[
  (none)
  (true)
  (false)
] @constant.builtin

(decorator
  "@" @attribute)
(decorator
  (identifier) @attribute)
(decorator
  (attribute
    attribute: (identifier) @attribute))
(decorator
  (call
    function: (identifier) @attribute))

(function_definition
  name: (identifier) @function)
(class_definition
  name: (identifier) @type)

((call
  function: (identifier) @function.builtin)
  (#match? @function.builtin "^(abs|all|any|ascii|bin|bool|breakpoint|bytearray|bytes|callable|chr|classmethod|compile|complex|delattr|dict|dir|divmod|enumerate|eval|exec|filter|float|format|frozenset|getattr|globals|hasattr|hash|help|hex|id|input|int|isinstance|issubclass|iter|len|list|locals|map|max|memoryview|min|next|object|oct|open|ord|pow|print|property|range|repr|reversed|round|set|setattr|slice|sorted|staticmethod|str|sum|super|tuple|type|vars|zip|__import__)$"))
(call
  function: (attribute
    attribute: (identifier) @function.method))
((call
  function: (identifier) @type)
  (#match? @type "^[A-Z]"))
(call
  function: (identifier) @function)

(type
  (identifier) @type)
(generic_type
  (identifier) @type)

((identifier) @variable.builtin
  (#match? @variable.builtin "^(self|cls)$"))

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))

((identifier) @type
  (#match? @type "^[A-Z]"))

(attribute
  attribute: (identifier) @property)
(keyword_argument
  name: (identifier) @variable.parameter)
(parameters
  (identifier) @variable.parameter)
(default_parameter
  name: (identifier) @variable.parameter)
(typed_parameter
  (identifier) @variable.parameter)
(typed_default_parameter
  name: (identifier) @variable.parameter)

[
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "exec"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "lambda"
  "nonlocal"
  "pass"
  "print"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
  "match"
  "case"
  "type"
  "and"
  "in"
  "is"
  "not"
  "or"
  "is not"
  "not in"
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
  ","
  "."
  ":"
  ";"
] @punctuation.delimiter

[
  "-"
  "-="
  "!="
  "*"
  "**"
  "**="
  "*="
  "/"
  "//"
  "//="
  "/="
  "&"
  "&="
  "%"
  "%="
  "^"
  "^="
  "+"
  "->"
  "+="
  "<"
  "<<"
  "<<="
  "<="
  "<>"
  "="
  ":="
  "=="
  ">"
  ">="
  ">>"
  ">>="
  "|"
  "|="
  "~"
  "@"
  "@="
] @operator

(identifier) @variable

; Sample: what colour each node is. A node with more than one capture takes the
; rightmost that the editor has a colour for, so a name written for a theme that
; has no style for it falls back to the capture on its left.

(comment) @comment
(string) @string
[
  (integer)
  (float)
] @number
[
  (none)
  (true)
  (false)
] @constant.builtin

(function_definition
  name: (identifier) @function)
(class_definition
  name: (identifier) @type)

; Names that start with a capital are types; `@sample.type` is no colour the
; editor knows, so these fall back to `@type`.
((identifier) @type @sample.type
  (#match? @type "^[A-Z][a-z]"))

; Names in capitals are constants; `@variable` is a colour, so it is preferred.
((identifier) @constant @variable
  (#match? @constant "^[A-Z][A-Z_0-9]+$"))

[
  "def"
  "class"
  "if"
  "elif"
  "else"
  "for"
  "while"
  "return"
  "import"
  "from"
  "in"
  "not"
  "and"
  "or"
] @keyword

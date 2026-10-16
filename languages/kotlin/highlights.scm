; Kotlin: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

[
  (line_comment)
  (multiline_comment)
  (shebang_line)
] @comment

(string_literal
  "$" @punctuation.special)
(string_literal
  "${" @punctuation.special
  "}" @punctuation.special)
(interpolated_identifier) @variable
[
  (string_literal)
  (character_literal)
] @string
(character_escape_seq) @string.escape

[
  (integer_literal)
  (long_literal)
  (hex_literal)
  (bin_literal)
  (unsigned_literal)
  (real_literal)
] @number

[
  "null"
  (boolean_literal)
] @constant.builtin

[
  (this_expression)
  (super_expression)
] @variable.builtin
((simple_identifier) @variable.builtin
  (#any-of? @variable.builtin "it" "field"))

(annotation
  "@" @attribute
  (use_site_target)? @attribute)
(annotation
  (user_type
    (type_identifier) @attribute))
(annotation
  (constructor_invocation
    (user_type
      (type_identifier) @attribute)))
(file_annotation
  "@" @attribute
  "file" @attribute
  ":" @attribute)
(file_annotation
  (user_type
    (type_identifier) @attribute))
(file_annotation
  (constructor_invocation
    (user_type
      (type_identifier) @attribute)))

(label) @label

(package_header
  (identifier
    (simple_identifier) @namespace))
(import_header
  (identifier
    (simple_identifier) @namespace))

(function_declaration
  (simple_identifier) @function)

(call_expression
  .
  (simple_identifier) @function.builtin
  (#any-of? @function.builtin "arrayOf" "arrayOfNulls" "emptyArray" "listOf" "setOf" "mapOf" "emptyList" "emptySet" "emptyMap" "mutableListOf" "mutableSetOf" "mutableMapOf" "print" "println" "error" "TODO" "run" "runCatching" "repeat" "lazy" "require" "requireNotNull" "check" "checkNotNull" "with" "apply" "also" "let" "takeIf"))
(call_expression
  .
  (simple_identifier) @function)
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @function.method) .))
(infix_expression
  (_)
  .
  (simple_identifier) @function.method
  .
  (_))
(callable_reference
  (simple_identifier) @function.method)

(getter
  "get" @keyword)
(setter
  "set" @keyword)
(secondary_constructor
  "constructor" @keyword)
(primary_constructor
  "constructor" @keyword)
(anonymous_initializer
  "init" @keyword)

(constructor_invocation
  (user_type
    (type_identifier) @constructor))

((type_identifier) @type.builtin
  (#any-of? @type.builtin "Byte" "Short" "Int" "Long" "UByte" "UShort" "UInt" "ULong" "Float" "Double" "Boolean" "Char" "String" "Unit" "Any" "Nothing"))
(type_identifier) @type

(enum_entry
  (simple_identifier) @constant)
((simple_identifier) @constant
  (#match? @constant "^_*[A-Z][A-Z0-9_]+$"))

(class_parameter
  (simple_identifier) @property)
(class_body
  (property_declaration
    (variable_declaration
      (simple_identifier) @property)))
(navigation_suffix
  (simple_identifier) @property)

(parameter
  (simple_identifier) @variable.parameter)
(parameter_with_optional_type
  (simple_identifier) @variable.parameter)
(lambda_parameters
  (variable_declaration
    (simple_identifier) @variable.parameter))

[
  (class_modifier)
  (member_modifier)
  (function_modifier)
  (property_modifier)
  (platform_modifier)
  (variance_modifier)
  (parameter_modifier)
  (visibility_modifier)
  (reification_modifier)
  (inheritance_modifier)
] @keyword

[
  "package"
  "import"
  "val"
  "var"
  "fun"
  "class"
  "interface"
  "object"
  "enum"
  "companion"
  "typealias"
  "by"
  "where"
  "if"
  "else"
  "when"
  "for"
  "do"
  "while"
  "try"
  "catch"
  "finally"
  "throw"
  "return"
  "return@"
  "break"
  "break@"
  "continue"
  "continue@"
] @keyword

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket
(type_arguments
  [
    "<"
    ">"
  ] @punctuation.bracket)
(type_parameters
  [
    "<"
    ">"
  ] @punctuation.bracket)

[
  "."
  ","
  ";"
  ":"
  "::"
] @punctuation.delimiter

[
  "!"
  "!="
  "!=="
  "="
  "=="
  "==="
  ">"
  ">="
  "<"
  "<="
  "||"
  "&&"
  "+"
  "++"
  "+="
  "-"
  "--"
  "-="
  "*"
  "*="
  "/"
  "/="
  "%"
  "%="
  "?."
  "?:"
  "!!"
  "is"
  "!is"
  "in"
  "!in"
  "as"
  "as?"
  ".."
  "->"
] @operator

(simple_identifier) @variable

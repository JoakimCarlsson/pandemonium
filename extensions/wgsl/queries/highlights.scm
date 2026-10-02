(identifier) @variable
[(line_comment) (block_comment)] @comment
[(int_literal) (float_literal)] @number
(bool_literal) @boolean
(type_declaration) @type
(struct_declaration name: (identifier) @type)
(function_declaration name: (identifier) @function)
(struct_member (variable_identifier_declaration name: (identifier) @property))
(parameter (variable_identifier_declaration name: (identifier) @variable.parameter))
(attribute) @attribute
(address_space) @keyword
(access_mode) @keyword
[
  "as" "bitcast" "break" "case" "continue" "continuing" "default"
  "discard" "else" "enable" "fallthrough" "fn" "for" "if" "let"
  "loop" "override" "return" "struct" "switch" "type" "var" "virtual" "while"
  "#define_import_path" "#else" "#endif" "#ifdef" "#ifndef" "#import"
] @keyword
[
  "!" "!=" "%" "%=" "&" "&&" "&=" "*" "*=" "+" "++" "+="
  "-" "--" "-=" "/" "/=" "<" "<<" "<=" "=" "==" ">" ">=" ">>"
  "^" "^=" "|" "|=" "||" "~"
] @operator
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":" "::" "." "->"] @punctuation.delimiter

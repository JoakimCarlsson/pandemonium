; PowerShell: what colour each node is. Where two patterns capture the same
; node, the one written first wins.

(comment) @comment

[
  (string_literal)
  (expandable_string_literal)
  (expandable_here_string_literal)
  (verbatim_string_characters)
  (verbatim_here_string_characters)
] @string

[
  (integer_literal)
  (real_literal)
] @number

(function_statement
  (function_name) @function)

(command
  command_name: (command_name) @function)

(invokation_expression
  (member_name) @function)

(member_access
  (member_name) @property)

(type_spec) @type

(command_parameter) @variable.parameter

((variable) @variable.builtin
  (#match? @variable.builtin "^\\$(_|true|false|null|args|input|PSItem|this|PWD|HOME|Host|PSScriptRoot|PSCommandPath|MyInvocation|Error|LASTEXITCODE|PSVersionTable|Matches|ErrorActionPreference|ProgressPreference|VerbosePreference)$"))
(variable) @variable

[
  "param"
  "dynamicparam"
  "begin"
  "process"
  "end"
  "if"
  "elseif"
  "else"
  "switch"
  "foreach"
  "for"
  "while"
  "do"
  "until"
  "function"
  "filter"
  "workflow"
  "break"
  "continue"
  "throw"
  "return"
  "exit"
  "trap"
  "try"
  "catch"
  "finally"
  "data"
  "inlinescript"
  "parallel"
  "sequence"
] @keyword

[
  "-as" "-is" "-isnot" "-in" "-notin" "-join" "-split" "-isplit" "-csplit"
  "-eq" "-ne" "-gt" "-ge" "-lt" "-le" "-ceq" "-cne" "-cgt" "-cge" "-clt" "-cle" "-ieq" "-ine" "-igt"
  "-ige" "-ilt" "-ile"
  "-like" "-notlike" "-clike" "-cnotlike" "-ilike" "-inotlike"
  "-match" "-notmatch" "-cmatch" "-cnotmatch" "-imatch" "-inotmatch"
  "-contains" "-notcontains" "-ccontains" "-cnotcontains" "-icontains" "-inotcontains"
  "-replace" "-creplace" "-ireplace"
  "-shl" "-shr"
  "-and" "-or" "-xor" "-not" "-band" "-bor" "-bxor"
  "+" "-" "*" "/" "\\" "%" ".."
] @operator

(command_invokation_operator) @operator

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
  "@("
  "$("
  "@{"
] @punctuation.bracket

";" @punctuation.delimiter

; Bash: what colour each node is. Where two patterns capture the same node,
; the one written first wins.

((program
  .
  (comment) @keyword.directive)
  (#match? @keyword.directive "^#![ \t]*/"))
(comment) @comment

[
  (string)
  (raw_string)
  (ansi_c_string)
  (translated_string)
  (heredoc_body)
] @string

[
  (heredoc_start)
  (heredoc_end)
] @label

[
  (regex)
  (extglob_pattern)
] @string.regexp

(number) @number

(special_variable_name) @constant.builtin

(function_definition
  name: (word) @function)

((command_name
  (word) @function.builtin)
  (#any-of? @function.builtin
    "." ":" "alias" "bg" "bind" "break" "builtin" "caller" "cd" "command" "compgen" "complete"
    "compopt" "continue" "coproc" "dirs" "disown" "echo" "enable" "eval" "exec" "exit" "false" "fc"
    "fg" "getopts" "hash" "help" "history" "jobs" "kill" "let" "logout" "mapfile" "popd" "printf"
    "pushd" "pwd" "read" "readarray" "return" "set" "shift" "shopt" "source" "suspend" "test" "time"
    "times" "trap" "true" "type" "typeset" "ulimit" "umask" "unalias" "wait"))
(command_name
  (word) @function)

(command_substitution
  "$(" @punctuation.special
  ")" @punctuation.special)
(process_substitution
  [
    "<("
    ">("
  ] @punctuation.special
  ")" @punctuation.special)
(arithmetic_expansion
  [
    "$(("
    "(("
  ] @punctuation.special
  "))" @punctuation.special)
(simple_expansion
  "$" @punctuation.special)
(expansion
  "${" @punctuation.special
  "}" @punctuation.special)
(expansion
  operator: _ @operator)

((variable_name) @variable.builtin
  (#any-of? @variable.builtin
    "CDPATH" "HOME" "IFS" "MAIL" "MAILPATH" "OPTARG" "OPTIND" "PATH" "PS1" "PS2" "PS3" "PS4"
    "BASH" "BASHOPTS" "BASHPID" "BASH_ARGC" "BASH_ARGV" "BASH_COMMAND" "BASH_LINENO"
    "BASH_REMATCH" "BASH_SOURCE" "BASH_SUBSHELL" "BASH_VERSINFO" "BASH_VERSION" "COLUMNS"
    "COMP_CWORD" "COMP_LINE" "COMP_POINT" "COMP_WORDS" "COMPREPLY" "DIRSTACK" "EUID" "FUNCNAME"
    "GROUPS" "HISTFILE" "HISTSIZE" "HOSTNAME" "HOSTTYPE" "LANG" "LC_ALL" "LINENO" "LINES"
    "MACHTYPE" "OLDPWD" "OSTYPE" "PIPESTATUS" "PPID" "PROMPT_COMMAND" "PWD" "RANDOM" "REPLY"
    "SECONDS" "SHELL" "SHELLOPTS" "SHLVL" "TMPDIR" "UID"))
((variable_name) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))
(variable_name) @variable

(file_descriptor) @operator
(test_operator) @operator
(binary_expression
  operator: _ @operator)
(unary_expression
  operator: _ @operator)
(postfix_expression
  operator: _ @operator)

((command
  argument: (word) @variable.parameter)
  (#match? @variable.parameter "^-"))

[
  "if"
  "then"
  "else"
  "elif"
  "fi"
  "case"
  "in"
  "esac"
  "for"
  "do"
  "done"
  "select"
  "until"
  "while"
  "function"
  "declare"
  "typeset"
  "readonly"
  "local"
  "export"
  "unset"
  "unsetenv"
] @keyword

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
  "[["
  "]]"
  "(("
  "))"
] @punctuation.bracket

[
  ";"
  ";;"
  ";&"
  ";;&"
  "&"
] @punctuation.delimiter

[
  ">"
  ">>"
  "<"
  "<<"
  "&&"
  "|"
  "|&"
  "||"
  "="
  "+="
  "=~"
  "=="
  "!="
  "&>"
  "&>>"
  "<&"
  ">&"
  ">|"
  "<&-"
  ">&-"
  "<<-"
  "<<<"
  "!"
] @operator

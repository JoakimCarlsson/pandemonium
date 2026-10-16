; C#: tokens that pair up, @open with the @close of the same match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
(type_argument_list
  "<" @open
  ">" @close)
(type_parameter_list
  "<" @open
  ">" @close)

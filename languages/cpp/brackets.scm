; C++: tokens that pair up, @open with the @close of the same match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
(template_argument_list
  "<" @open
  ">" @close)
(template_parameter_list
  "<" @open
  ">" @close)

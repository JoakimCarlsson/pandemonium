; Nix: tokens that pair up, @open with the @close of the same match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
("${" @open "}" @close)
(indented_string_expression
  "''" @open
  "''" @close)

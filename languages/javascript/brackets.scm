; JavaScript: tokens that pair up, @open with the @close of the same match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
(template_substitution "${" @open "}" @close)
(template_string "`" @open "`" @close)
(jsx_opening_element "<" @open ">" @close)
(jsx_self_closing_element "<" @open "/>" @close)
(jsx_closing_element "</" @open ">" @close)

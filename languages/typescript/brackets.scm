; TypeScript: tokens that pair up, @open with the @close of the same match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
(template_substitution "${" @open "}" @close)
(template_string "`" @open "`" @close)
(type_arguments "<" @open ">" @close)
(type_parameters "<" @open ">" @close)

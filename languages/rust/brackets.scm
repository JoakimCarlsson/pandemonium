; Rust: tokens that pair up, @open with the @close of the same match.

("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)
(type_arguments "<" @open ">" @close)
(type_parameters "<" @open ">" @close)
(closure_parameters "|" @open "|" @close)

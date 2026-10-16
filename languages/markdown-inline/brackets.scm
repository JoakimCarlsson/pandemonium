; Inline markdown: tokens that pair up, @open with the @close of the same
; match.

(inline_link "[" @open "]" @close)
(inline_link "(" @open ")" @close)
(shortcut_link "[" @open "]" @close)
(image "[" @open "]" @close)
(image "(" @open ")" @close)

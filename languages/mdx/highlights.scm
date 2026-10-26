; Markdown: what colour each block is. The text inside a block is the inline
; grammar's, injected below. Where two patterns capture the same node, the one
; written first wins.

(atx_heading
  (inline) @markup.heading)
(setext_heading
  (paragraph) @markup.heading)

[
  (atx_h1_marker)
  (atx_h2_marker)
  (atx_h3_marker)
  (atx_h4_marker)
  (atx_h5_marker)
  (atx_h6_marker)
  (setext_h1_underline)
  (setext_h2_underline)
] @markup.heading

[
  (indented_code_block)
  (fenced_code_block)
] @markup.raw

(fenced_code_block_delimiter) @punctuation.delimiter
(info_string) @label

(link_destination) @markup.link
(link_label) @markup.link
(link_title) @string

[
  (list_marker_plus)
  (list_marker_minus)
  (list_marker_star)
  (list_marker_dot)
  (list_marker_parenthesis)
  (thematic_break)
  (block_quote_marker)
] @punctuation.special

(backslash_escape) @string.escape

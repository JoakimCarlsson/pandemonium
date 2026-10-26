; Markdown: the items an outline lists. @item is the whole item, @name what it
; is called and @context the words that say what it is.

(section
  (atx_heading
    [
      (atx_h1_marker)
      (atx_h2_marker)
      (atx_h3_marker)
      (atx_h4_marker)
      (atx_h5_marker)
      (atx_h6_marker)
    ] @context
    heading_content: (_) @name)) @item

(section
  (setext_heading
    (paragraph) @name)) @item

; HTML: tokens that pair up, @open with the @close of the same match. The start
; and end tags of an element pair up by their names.

(element
  (start_tag
    (tag_name) @open)
  (end_tag
    (tag_name) @close))

(script_element
  (start_tag
    (tag_name) @open)
  (end_tag
    (tag_name) @close))

(style_element
  (start_tag
    (tag_name) @open)
  (end_tag
    (tag_name) @close))

(self_closing_tag
  "<" @open
  "/>" @close)

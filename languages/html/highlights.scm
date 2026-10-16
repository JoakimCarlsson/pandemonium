; HTML: what colour each node is. The styles and scripts inside a page are
; their own languages', injected. Where two patterns capture the same node, the
; one written first wins.

(comment) @comment

(doctype) @keyword
"<!" @punctuation.bracket

(tag_name) @tag
(erroneous_end_tag_name) @tag

(attribute_name) @attribute

(attribute
  (attribute_name) @_name
  (quoted_attribute_value
    (attribute_value) @markup.link)
  (#any-of? @_name "href" "src"))

(quoted_attribute_value) @string
(attribute_value) @string

(entity) @string.escape

((element
  (start_tag
    (tag_name) @_tag)
  (text) @markup.heading)
  (#any-of? @_tag "title" "h1" "h2" "h3" "h4" "h5" "h6"))

((element
  (start_tag
    (tag_name) @_tag)
  (text) @markup.strong)
  (#any-of? @_tag "strong" "b"))

((element
  (start_tag
    (tag_name) @_tag)
  (text) @markup.italic)
  (#any-of? @_tag "em" "i"))

((element
  (start_tag
    (tag_name) @_tag)
  (text) @markup.raw)
  (#any-of? @_tag "code" "kbd" "pre" "samp"))

((element
  (start_tag
    (tag_name) @_tag)
  (text) @markup.link)
  (#eq? @_tag "a"))

[
  "<"
  ">"
  "</"
  "/>"
] @punctuation.bracket

"=" @operator

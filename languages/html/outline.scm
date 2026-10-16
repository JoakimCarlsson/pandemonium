; HTML: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. Headings are listed by
; their text, and other elements with an id by the id.

(element
  (start_tag
    (tag_name) @context)
  .
  (text) @name
  (#any-of? @context "h1" "h2" "h3" "h4" "h5" "h6")) @item

(element
  (start_tag
    (tag_name) @context
    (attribute
      (attribute_name) @_attribute
      [
        (quoted_attribute_value
          (attribute_value) @name)
        (attribute_value) @name
      ]))
  (#eq? @_attribute "id")
  (#not-any-of? @context "h1" "h2" "h3" "h4" "h5" "h6")) @item

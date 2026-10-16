; Svelte: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. The script and style of a
; component are listed by their tags, snippets by their names, and elements
; with an id by the id.

(script_element
  (start_tag
    (tag_name) @name)) @item

(style_element
  (start_tag
    (tag_name) @name)) @item

(snippet_statement
  (snippet_start
    (block_start_tag) @context
    (snippet_name) @name)) @item

(element
  (start_tag
    (tag_name) @context
    (attribute
      (attribute_name) @_attribute
      (quoted_attribute_value
        (attribute_value) @name)))
  (#eq? @_attribute "id")) @item

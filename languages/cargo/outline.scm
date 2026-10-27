; TOML: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(table
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @name) @item

(table_array_element
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @name) @item

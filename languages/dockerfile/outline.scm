; Dockerfile: the items an outline lists. @item is the whole item, @name what
; it is called and @context the words that say what it is. A build stage is
; named by its alias when it has one, and by its image when it has not.

(from_instruction
  "FROM" @context
  as: (image_alias) @name) @item

(from_instruction
  "FROM" @context
  (image_spec) @name
  .) @item

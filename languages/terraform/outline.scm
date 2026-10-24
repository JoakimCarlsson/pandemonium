; Terraform: the items an outline lists. @item is the whole item, @name what it
; is called and @context the words that say what it is. A block is named by its
; labels; a block with none is listed only at the top of the file, by its type,
; and each value of a locals block is listed under it.

(block
  .
  (identifier) @context
  .
  [
    (string_lit
      (template_literal) @name)
    (identifier) @name
  ]+
  .
  (block_start)) @item

(config_file
  (body
    (block
      .
      (identifier) @name
      .
      (block_start)) @item))

(config_file
  (body
    (attribute
      (identifier) @name) @item))

(block
  .
  (identifier) @_locals
  (body
    (attribute
      (identifier) @name) @item)
  (#eq? @_locals "locals"))

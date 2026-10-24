; Python: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(class_definition
  "class" @context
  name: (identifier) @name) @item

(function_definition
  "async"? @context
  "def" @context
  name: (identifier) @name) @item

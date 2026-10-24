; Bash: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(function_definition
  "function"? @context
  name: (word) @name) @item

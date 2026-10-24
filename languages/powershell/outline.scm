; PowerShell: the items an outline lists. @item is the whole item, @name what it
; is called and @context the words that say what it is.

(function_statement
  [
    "function"
    "filter"
    "workflow"
  ] @context
  (function_name) @name) @item

(class_statement
  (simple_name) @name) @item

(enum_statement
  (simple_name) @name) @item

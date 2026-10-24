; JavaScript: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(class_declaration
  "class" @context
  name: (_) @name) @item

(function_declaration
  "async"? @context
  "function" @context
  name: (_) @name) @item

(generator_function_declaration
  "async"? @context
  "function" @context
  name: (_) @name) @item

(method_definition
  name: (_) @name) @item

(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: [(arrow_function) (function_expression)]) @item)

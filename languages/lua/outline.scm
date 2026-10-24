; Lua: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(function_declaration
  "local"? @context
  "global"? @context
  "function" @context
  name: (_) @name) @item

(assignment_statement
  (variable_list
    .
    name: (_) @name)
  (expression_list
    .
    value: (function_definition
      "function" @context))) @item

(variable_declaration
  "local" @context
  (assignment_statement
    (variable_list
      .
      name: (_) @name)
    (expression_list
      .
      value: (function_definition
        "function" @context)))) @item

(field
  name: (identifier) @name
  value: (function_definition
    "function" @context)) @item

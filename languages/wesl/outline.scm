; WESL: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(function_decl
  (function_header
    "fn" @context
    name: (identifier) @name)) @item

(struct_decl
  "struct" @context
  name: (identifier) @name) @item

(type_alias_decl
  "alias" @context
  name: (identifier) @name) @item

(global_value_decl
  ["const" "override"] @context
  name: (identifier) @name) @item

(global_variable_decl
  (variable_decl
    "var" @context
    name: (identifier) @name)) @item

; Go: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(function_declaration
  "func" @context
  name: (identifier) @name) @item

(method_declaration
  "func" @context
  receiver: (parameter_list
    (parameter_declaration
      type: (_) @context))
  name: (field_identifier) @name) @item

(type_declaration
  "type" @context
  (type_spec
    name: (type_identifier) @name)) @item

(const_declaration
  "const" @context
  (const_spec
    name: (identifier) @name)) @item

(var_declaration
  "var" @context
  (var_spec
    name: (identifier) @name)) @item

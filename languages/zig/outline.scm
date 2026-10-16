; Zig: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(function_declaration
  "pub"? @context
  "fn" @context
  name: (identifier) @name) @item

(variable_declaration
  "pub"? @context
  [
    "const"
    "var"
  ] @context
  (identifier) @name
  [
    (struct_declaration
      "struct" @context)
    (enum_declaration
      "enum" @context)
    (union_declaration
      "union" @context)
    (opaque_declaration
      "opaque" @context)
    (error_set_declaration
      "error" @context)
  ]) @item

(test_declaration
  "test" @context
  [
    (string)
    (identifier)
  ] @name) @item

(container_field
  name: (identifier) @name) @item

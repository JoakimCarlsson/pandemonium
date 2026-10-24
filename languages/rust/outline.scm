; Rust: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(mod_item
  "mod" @context
  name: (_) @name) @item

(struct_item
  "struct" @context
  name: (_) @name) @item

(enum_item
  "enum" @context
  name: (_) @name) @item

(union_item
  "union" @context
  name: (_) @name) @item

(trait_item
  "trait" @context
  name: (_) @name) @item

(type_item
  "type" @context
  name: (_) @name) @item

(impl_item
  "impl" @context
  trait: (_)? @name
  "for"? @context
  type: (_) @name) @item

(function_item
  "fn" @context
  name: (_) @name) @item

(function_signature_item
  "fn" @context
  name: (_) @name) @item

(macro_definition
  "macro_rules!" @context
  name: (_) @name) @item

(const_item
  "const" @context
  name: (_) @name) @item

(static_item
  "static" @context
  name: (_) @name) @item

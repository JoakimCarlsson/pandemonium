; Swift: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(class_declaration
  declaration_kind: _ @context
  name: (_) @name) @item

(protocol_declaration
  "protocol" @context
  name: (_) @name) @item

(enum_entry
  "case" @context
  name: (simple_identifier) @name @item)

(typealias_declaration
  "typealias" @context
  name: (type_identifier) @name) @item

(associatedtype_declaration
  "associatedtype" @context
  name: (_) @name) @item

(init_declaration
  name: "init" @name) @item

(deinit_declaration
  "deinit" @name) @item

(subscript_declaration
  "subscript" @name) @item

(function_declaration
  "func" @context
  name: [
    (simple_identifier)
    (custom_operator)
  ] @name) @item

(protocol_function_declaration
  "func" @context
  name: [
    (simple_identifier)
    (custom_operator)
  ] @name) @item

(macro_declaration
  "macro" @context
  (simple_identifier) @name) @item

(source_file
  (property_declaration
    (value_binding_pattern) @context
    .
    name: (pattern) @name) @item)

(class_body
  (property_declaration
    (value_binding_pattern) @context
    .
    name: (pattern) @name) @item)

(enum_class_body
  (property_declaration
    (value_binding_pattern) @context
    .
    name: (pattern) @name) @item)

(protocol_property_declaration
  name: (pattern
    (simple_identifier) @name)
  (type_annotation
    (_) @context)) @item

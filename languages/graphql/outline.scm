; GraphQL: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. Operations, fragments,
; types and directives are listed, and the fields of a type under it.

(operation_definition
  (operation_type) @context
  (name) @name) @item

(fragment_definition
  "fragment" @context
  (fragment_name
    (name) @name)) @item

(schema_definition
  "schema" @name) @item

(object_type_definition
  "type" @context
  (name) @name) @item

(interface_type_definition
  "interface" @context
  (name) @name) @item

(input_object_type_definition
  "input" @context
  (name) @name) @item

(enum_type_definition
  "enum" @context
  (name) @name) @item

(union_type_definition
  "union" @context
  (name) @name) @item

(scalar_type_definition
  "scalar" @context
  (name) @name) @item

(object_type_extension
  "extend" @context
  "type" @context
  (name) @name) @item

(interface_type_extension
  "extend" @context
  "interface" @context
  (name) @name) @item

(input_object_type_extension
  "extend" @context
  "input" @context
  (name) @name) @item

(directive_definition
  "directive" @context
  (name) @name) @item

(field_definition
  (name) @name) @item

(input_fields_definition
  (input_value_definition
    (name) @name) @item)

(enum_value_definition
  (enum_value
    (name) @name)) @item

; Kotlin: the items an outline lists. @item is the whole item, @name what it
; is called and @context the words that say what it is.

(package_header
  "package" @context
  (identifier) @name) @item

(class_declaration
  (modifiers
    (class_modifier) @context)?
  "enum"? @context
  [
    "class"
    "interface"
  ] @context
  (type_identifier) @name) @item

(object_declaration
  "object" @context
  (type_identifier) @name) @item

(companion_object
  "companion" @context
  "object" @context
  (type_identifier) @name) @item

(companion_object
  "companion" @name
  "object" @name
  .
  (class_body)) @item

(enum_class_body
  (enum_entry
    (simple_identifier) @name) @item)

(type_alias
  "typealias" @context
  (type_identifier) @name) @item

(secondary_constructor
  "constructor" @name) @item

(function_declaration
  "fun" @context
  (simple_identifier) @name) @item

(source_file
  (property_declaration
    (binding_pattern_kind) @context
    (variable_declaration
      (simple_identifier) @name)) @item)

(class_body
  (property_declaration
    (binding_pattern_kind) @context
    (variable_declaration
      (simple_identifier) @name)) @item)

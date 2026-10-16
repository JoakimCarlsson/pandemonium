; Java: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(package_declaration
  "package" @context
  (_) @name) @item

(module_declaration
  "module" @context
  name: (_) @name) @item

(class_declaration
  "class" @context
  name: (_) @name) @item

(interface_declaration
  "interface" @context
  name: (_) @name) @item

(annotation_type_declaration
  "@interface" @context
  name: (_) @name) @item

(enum_declaration
  "enum" @context
  name: (_) @name) @item

(record_declaration
  "record" @context
  name: (_) @name) @item

(enum_constant
  name: (_) @name) @item

(constructor_declaration
  name: (_) @name) @item

(compact_constructor_declaration
  name: (_) @name) @item

(method_declaration
  type: (_) @context
  name: (_) @name) @item

(annotation_type_element_declaration
  type: (_) @context
  name: (_) @name) @item

(field_declaration
  type: (_) @context
  declarator: (variable_declarator
    name: (_) @name)) @item

(constant_declaration
  type: (_) @context
  declarator: (variable_declarator
    name: (_) @name)) @item

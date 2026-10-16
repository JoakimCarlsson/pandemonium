; C#: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(namespace_declaration
  "namespace" @context
  name: (_) @name) @item

(file_scoped_namespace_declaration
  "namespace" @context
  name: (_) @name) @item

(class_declaration
  "class" @context
  name: (_) @name) @item

(struct_declaration
  "struct" @context
  name: (_) @name) @item

(record_declaration
  "record" @context
  "class"? @context
  "struct"? @context
  name: (_) @name) @item

(interface_declaration
  "interface" @context
  name: (_) @name) @item

(enum_declaration
  "enum" @context
  name: (_) @name) @item

(enum_member_declaration
  name: (_) @name) @item

(delegate_declaration
  "delegate" @context
  type: (_) @context
  name: (_) @name) @item

(constructor_declaration
  name: (_) @name) @item

(destructor_declaration
  "~" @context
  name: (_) @name) @item

(method_declaration
  returns: (_) @context
  name: (_) @name) @item

(local_function_statement
  type: (_) @context
  name: (_) @name) @item

(operator_declaration
  type: (_) @context
  "operator" @name
  operator: _ @name) @item

(conversion_operator_declaration
  [
    "implicit"
    "explicit"
  ] @context
  "operator" @name
  type: (_) @name) @item

(indexer_declaration
  type: (_) @context
  "this" @name) @item

(property_declaration
  type: (_) @context
  name: (_) @name) @item

(event_declaration
  "event" @context
  type: (_) @context
  name: (_) @name) @item

(event_field_declaration
  "event" @context
  (variable_declaration
    type: (_) @context
    (variable_declarator
      name: (_) @name))) @item

(field_declaration
  (variable_declaration
    type: (_) @context
    (variable_declarator
      name: (_) @name))) @item

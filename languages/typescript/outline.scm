; TypeScript: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(class_declaration
  "class" @context
  name: (_) @name) @item

(function_declaration
  "async"? @context
  "function" @context
  name: (_) @name) @item

(generator_function_declaration
  "async"? @context
  "function" @context
  name: (_) @name) @item

(method_definition
  name: (_) @name) @item

(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: [(arrow_function) (function_expression)]) @item)

(abstract_class_declaration
  "abstract" @context
  "class" @context
  name: (_) @name) @item

(interface_declaration
  "interface" @context
  name: (_) @name) @item

(type_alias_declaration
  "type" @context
  name: (_) @name) @item

(enum_declaration
  "enum" @context
  name: (_) @name) @item

(internal_module
  "namespace" @context
  name: (_) @name) @item

(function_signature
  "function" @context
  name: (_) @name) @item

(method_signature
  name: (_) @name) @item

(abstract_method_signature
  name: (_) @name) @item

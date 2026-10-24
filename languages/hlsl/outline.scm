; HLSL: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. A constant buffer outside
; a function reads as a function or a variable named after it, so it is listed
; by the type it seems to have.

(declaration
  type: (type_identifier) @context
  .
  declarator: (identifier) @name
  (#eq? @context "cbuffer")) @item

(function_definition
  type: (type_identifier) @context
  declarator: (identifier) @name
  (#eq? @context "cbuffer")) @item

(cbuffer_specifier
  "cbuffer" @context
  name: (_) @name) @item

(namespace_definition
  "namespace" @context
  name: (_) @name) @item

(function_definition
  type: (_) @context
  declarator: (function_declarator
    declarator: (_) @name)) @item

(struct_specifier
  "struct" @context
  name: (type_identifier) @name
  body: (_)) @item

(class_specifier
  "class" @context
  name: (type_identifier) @name
  body: (_)) @item

(enum_specifier
  "enum" @context
  name: (type_identifier) @name
  body: (_)) @item

(preproc_def
  "#define" @context
  name: (identifier) @name) @item

(preproc_function_def
  "#define" @context
  name: (identifier) @name) @item

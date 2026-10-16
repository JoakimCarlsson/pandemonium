; C++: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(namespace_definition
  "namespace" @context
  name: (_) @name) @item

(class_specifier
  "class" @context
  name: (_) @name
  body: (_)) @item

(struct_specifier
  "struct" @context
  name: (_) @name
  body: (_)) @item

(union_specifier
  "union" @context
  name: (_) @name
  body: (_)) @item

(enum_specifier
  "enum" @context
  "class"? @context
  "struct"? @context
  name: (_) @name
  body: (_)) @item

(concept_definition
  "concept" @context
  name: (_) @name) @item

(alias_declaration
  "using" @context
  name: (_) @name) @item

(type_definition
  "typedef" @context
  declarator: (_) @name) @item

(preproc_def
  "#define" @context
  name: (_) @name) @item

(preproc_function_def
  "#define" @context
  name: (_) @name) @item

(function_definition
  type: (_)? @context
  declarator: (function_declarator
    declarator: (_) @name)) @item

(function_definition
  type: (_)? @context
  declarator: (pointer_declarator
    "*" @context
    declarator: (function_declarator
      declarator: (_) @name))) @item

(function_definition
  type: (_)? @context
  declarator: (reference_declarator
    "&" @context
    (function_declarator
      declarator: (_) @name))) @item

(declaration
  type: (_)? @context
  declarator: (function_declarator
    declarator: (_) @name)) @item

(field_declaration
  type: (_)? @context
  declarator: (function_declarator
    declarator: (_) @name)) @item

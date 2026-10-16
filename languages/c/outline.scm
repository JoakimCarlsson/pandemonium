; C: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

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
  name: (_) @name
  body: (_)) @item

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
  type: (_) @context
  declarator: (function_declarator
    declarator: (identifier) @name)) @item

(function_definition
  type: (_) @context
  declarator: (pointer_declarator
    "*" @context
    declarator: (function_declarator
      declarator: (identifier) @name))) @item

(declaration
  type: (_) @context
  declarator: (function_declarator
    declarator: (identifier) @name)) @item

(declaration
  type: (_) @context
  declarator: (pointer_declarator
    "*" @context
    declarator: (function_declarator
      declarator: (identifier) @name))) @item

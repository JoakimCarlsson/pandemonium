; GLSL: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(function_definition
  type: (_) @context
  declarator: (function_declarator
    declarator: (identifier) @name)) @item

(struct_specifier
  "struct" @context
  name: (type_identifier) @name
  body: (_)) @item

(declaration
  [
    "uniform"
    "buffer"
    "in"
    "out"
    "shared"
  ] @context
  .
  (identifier) @name
  .
  (field_declaration_list)) @item

(preproc_def
  "#define" @context
  name: (identifier) @name) @item

(preproc_function_def
  "#define" @context
  name: (identifier) @name) @item

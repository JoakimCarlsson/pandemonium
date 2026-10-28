; Rust: whole functions, types and comments to select.

(function_item
  body: (_) @function.inside) @function.around

(function_signature_item) @function.around

(struct_item
  body: (_) @class.inside) @class.around

(enum_item
  body: (_) @class.inside) @class.around

(trait_item
  body: (_) @class.inside) @class.around

(impl_item
  body: (_) @class.inside) @class.around

(mod_item
  body: (_) @class.inside) @class.around

[
  (line_comment)
  (block_comment)
]+ @comment.around

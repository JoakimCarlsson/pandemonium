; Go: whole functions, types and comments to select.

(function_declaration
  body: (_) @function.inside) @function.around

(method_declaration
  body: (_) @function.inside) @function.around

(type_declaration) @class.around

(comment)+ @comment.around

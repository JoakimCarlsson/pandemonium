; TSX: whole functions, classes and comments to select.

(function_declaration
  body: (_) @function.inside) @function.around

(generator_function_declaration
  body: (_) @function.inside) @function.around

(method_definition
  body: (_) @function.inside) @function.around

(class_declaration
  body: (_) @class.inside) @class.around

(comment)+ @comment.around

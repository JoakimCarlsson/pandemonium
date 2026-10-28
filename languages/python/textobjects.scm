; Python: whole functions, classes and comments to select.

(function_definition
  body: (_) @function.inside) @function.around

(class_definition
  body: (_) @class.inside) @class.around

(comment)+ @comment.around

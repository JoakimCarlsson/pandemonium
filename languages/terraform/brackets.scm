; Terraform: tokens that pair up, @open with the @close of the same match.

(block
  (block_start) @open
  (block_end) @close)
(object
  (object_start) @open
  (object_end) @close)
(tuple
  (tuple_start) @open
  (tuple_end) @close)
(for_object_expr
  (object_start) @open
  (object_end) @close)
(for_tuple_expr
  (tuple_start) @open
  (tuple_end) @close)
(template_interpolation
  (template_interpolation_start) @open
  (template_interpolation_end) @close)
(template_directive
  (_
    (template_directive_start) @open
    (template_directive_end) @close))
("(" @open ")" @close)
("[" @open "]" @close)

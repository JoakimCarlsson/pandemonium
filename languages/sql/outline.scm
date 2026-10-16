; SQL: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is.

(create_table
  (keyword_table) @context
  (object_reference) @name) @item

(create_view
  (keyword_view) @context
  (object_reference) @name) @item

(create_materialized_view
  (keyword_materialized) @context
  (keyword_view) @context
  (object_reference) @name) @item

(create_function
  (keyword_function) @context
  (object_reference) @name) @item

(create_type
  (keyword_type) @context
  (object_reference) @name) @item

(create_sequence
  (keyword_sequence) @context
  (object_reference) @name) @item

(create_index
  (keyword_index) @context
  column: (_) @name) @item

(create_trigger
  (keyword_trigger) @context
  (object_reference) @name
  [
    (keyword_before)
    (keyword_after)
    (keyword_instead)
  ]) @item

(create_schema
  (keyword_schema) @context
  (identifier) @name) @item

(cte
  .
  (identifier) @name) @item

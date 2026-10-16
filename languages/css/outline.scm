; CSS: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. A rule is listed by its
; selectors, and an at-rule by its keyword and what follows it.

(rule_set
  (selectors) @name) @item

(media_statement
  "@media" @context
  .
  [
    (keyword_query)
    (feature_query)
    (binary_query)
    (unary_query)
    (selector_query)
    (parenthesized_query)
  ] @name) @item

(supports_statement
  "@supports" @context
  .
  [
    (keyword_query)
    (feature_query)
    (binary_query)
    (unary_query)
    (selector_query)
    (parenthesized_query)
  ] @name) @item

(keyframes_statement
  [
    "@keyframes"
    (at_keyword)
  ] @context
  (keyframes_name) @name) @item

(keyframe_block
  [
    (from)
    (to)
    (integer_value)
  ] @name) @item

(at_rule
  (at_keyword) @context
  .
  [
    (keyword_query)
    (feature_query)
    (binary_query)
    (unary_query)
    (selector_query)
    (parenthesized_query)
  ] @name) @item

(at_rule
  .
  (at_keyword) @name
  .
  (block)) @item

(import_statement
  "@import" @context
  .
  (_) @name) @item

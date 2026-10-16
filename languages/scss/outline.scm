; SCSS: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. A rule is listed by its
; selectors, a mixin or function by its name, a variable set at the top of the
; file by its name, and an at-rule by its keyword and what follows it.

(rule_set
  (selectors) @name) @item

(mixin_statement
  "@mixin" @context
  name: (identifier) @name) @item

(function_statement
  "@function" @context
  name: (identifier) @name) @item

(stylesheet
  (declaration
    (property_name) @name
    (#match? @name "^\\$")) @item)

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

(use_statement
  "@use" @context
  .
  (_) @name) @item

(forward_statement
  "@forward" @context
  .
  (_) @name) @item

(import_statement
  "@import" @context
  .
  (_) @name) @item

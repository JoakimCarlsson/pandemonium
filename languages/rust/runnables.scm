; Rust: the places that can be run. @run.<kind> is the whole place and @name
; what it is called.

((function_item
  name: (identifier) @name) @run.main
  (#eq? @name "main"))

(
  (attribute_item
    (attribute
      (identifier) @attribute
      (#any-of? @attribute "test" "rstest")))
  .
  (function_item
    name: (identifier) @name) @run.test)

(
  (attribute_item
    (attribute
      (scoped_identifier
        name: (identifier) @attribute
        (#eq? @attribute "test"))))
  .
  (function_item
    name: (identifier) @name) @run.test)

(
  (attribute_item
    (attribute
      (identifier) @attribute
      arguments: (token_tree
        (identifier) @condition)
      (#eq? @attribute "cfg")
      (#eq? @condition "test")))
  .
  (mod_item
    name: (identifier) @name) @run.module)

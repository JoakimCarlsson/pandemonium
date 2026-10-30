; Rust: the places that can be run. @run is the whole place, its tag says what
; kind of place it is and @name, read as $PM_CUSTOM_name, what it is called.

((function_item
  name: (identifier) @name) @run
  (#eq? @name "main")
  (#set! tag rust-main))

(
  (attribute_item
    (attribute
      (identifier) @_attribute
      (#any-of? @_attribute "test" "rstest")))
  .
  (function_item
    name: (identifier) @name) @run
  (#set! tag rust-test))

(
  (attribute_item
    (attribute
      (scoped_identifier
        name: (identifier) @_attribute
        (#eq? @_attribute "test"))))
  .
  (function_item
    name: (identifier) @name) @run
  (#set! tag rust-test))

(
  (attribute_item
    (attribute
      (identifier) @_attribute
      arguments: (token_tree
        (identifier) @_condition)
      (#eq? @_attribute "cfg")
      (#eq? @_condition "test")))
  .
  (mod_item
    name: (identifier) @name) @run
  (#set! tag rust-module))

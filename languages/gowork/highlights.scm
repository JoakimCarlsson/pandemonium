; go.work: what colour each node is.

(comment) @comment

[
  "go"
  "use"
  "replace"
] @keyword

"=>" @operator

[
  (interpreted_string_literal)
  (raw_string_literal)
] @string

(module_path) @type
(version) @string
(go_version) @string

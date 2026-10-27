; go.mod: what colour each node is.

(comment) @comment

[
  "module"
  "go"
  "toolchain"
  "require"
  "replace"
  "exclude"
  "retract"
] @keyword

"=>" @operator

[
  (interpreted_string_literal)
  (raw_string_literal)
] @string

(module_path) @type
(version) @string
(go_version) @string

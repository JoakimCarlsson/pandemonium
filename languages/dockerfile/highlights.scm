; Dockerfile: what colour each node is. Where two patterns capture the same
; node, the one written first wins.

(comment) @comment

[
  "FROM"
  "AS"
  "RUN"
  "CMD"
  "LABEL"
  "EXPOSE"
  "ENV"
  "ADD"
  "COPY"
  "ENTRYPOINT"
  "VOLUME"
  "USER"
  "WORKDIR"
  "ARG"
  "ONBUILD"
  "STOPSIGNAL"
  "HEALTHCHECK"
  "SHELL"
  "MAINTAINER"
  "CROSS_BUILD"
] @keyword

[
  (heredoc_marker)
  (heredoc_end)
] @label

[
  (double_quoted_string)
  (single_quoted_string)
  (json_string)
] @string
(add_instruction
  (heredoc_block
    (heredoc_line) @string))
(copy_instruction
  (heredoc_block
    (heredoc_line) @string))
(escape_sequence) @string.escape

(image_spec
  (image_name) @type)
(image_spec
  (image_tag) @constant)
(image_spec
  (image_digest) @constant)
(image_alias) @label

(expose_port) @number

(param) @attribute
(mount_param
  "--" @attribute
  name: _ @attribute)

(expansion
  [
    "$"
    "{"
    "}"
  ] @punctuation.special)
((variable) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))
(variable) @variable

(arg_instruction
  .
  (unquoted_string) @property)
(env_pair
  name: (unquoted_string) @property)
(label_pair
  key: (_) @property)

[
  "="
  ":"
  "@"
] @operator

[
  "["
  "]"
] @punctuation.bracket

"," @punctuation.delimiter

(line_continuation) @punctuation.special

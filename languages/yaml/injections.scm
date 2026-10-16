; YAML: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; commands of a CI job, under keys such as run and script, are shell.

(block_mapping_pair
  key: (flow_node) @_key
  value: (flow_node
    (plain_scalar
      (string_scalar) @injection.content))
  (#any-of? @_key "run" "script" "before_script" "after_script" "cmd" "cmds" "sh")
  (#set! injection.language "bash"))

(block_mapping_pair
  key: (flow_node) @_key
  value: (block_node
    (block_scalar) @injection.content)
  (#any-of? @_key "run" "script" "before_script" "after_script" "cmd" "cmds" "sh")
  (#set! injection.language "bash"))

(block_mapping_pair
  key: (flow_node) @_key
  value: (block_node
    (block_sequence
      (block_sequence_item
        (flow_node
          (plain_scalar
            (string_scalar) @injection.content)))))
  (#any-of? @_key "script" "before_script" "after_script" "cmds")
  (#set! injection.language "bash"))

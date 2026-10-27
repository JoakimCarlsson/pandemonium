; Compose: the places that carry a lens. @lens.<kind> is the whole place and
; @name what it is called.

; The `services` key, which runs every service.
(stream
  (document
    (block_node
      (block_mapping
        (block_mapping_pair
          key: (flow_node
            (plain_scalar
              (string_scalar) @_section))) @lens.all)))
  (#eq? @_section "services"))

; Each service under it.
(stream
  (document
    (block_node
      (block_mapping
        (block_mapping_pair
          key: (flow_node
            (plain_scalar
              (string_scalar) @_section))
          value: (block_node
            (block_mapping
              (block_mapping_pair
                key: (flow_node
                  (plain_scalar
                    (string_scalar) @name))) @lens.service))))))
  (#eq? @_section "services"))

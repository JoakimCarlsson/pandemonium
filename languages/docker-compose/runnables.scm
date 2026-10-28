; Docker Compose: each service of a compose file is a place that can be run.
; It is tagged docker-compose-service and its name is $PM_CUSTOM_service.

(
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
                      (string_scalar) @service))) @run)))))))
  (#eq? @_section "services")
  (#set! tag docker-compose-service))

; The services key of a compose file is the place that runs or stops them all.
; It is tagged docker-compose-services.

(
  (stream
    (document
      (block_node
        (block_mapping
          (block_mapping_pair
            key: (flow_node
              (plain_scalar
                (string_scalar) @_section)) @run)))))
  (#eq? @_section "services")
  (#set! tag docker-compose-services))

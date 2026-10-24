; Lua: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; string handed to ffi.cdef declares C.

((function_call
  name: (dot_index_expression
    field: (identifier) @_cdef)
  arguments: (arguments
    (string
      content: (_) @injection.content)))
  (#eq? @_cdef "cdef")
  (#set! injection.language "c"))

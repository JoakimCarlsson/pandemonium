; C: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; body of a macro is C itself.

((preproc_arg) @injection.content
  (#set! injection.language "c"))

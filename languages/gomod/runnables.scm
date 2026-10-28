; go.mod: the places that have links at the end of their line. The module line
; is tagged go-mod-module and each require directive go-mod-require.

((module_directive) @run
  (#set! tag go-mod-module))

((require_directive) @run
  (#set! tag go-mod-require))

; Nix: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; strings bound to the phases of a build, the hooks run before and after them,
; a script and a shell hook hold Bash.

((binding
  attrpath: (attrpath
    .
    (identifier) @_name .)
  expression: [
    (string_expression
      (string_fragment) @injection.content)
    (indented_string_expression
      (string_fragment) @injection.content)
  ])
  (#match? @_name "^([a-z]+Phase|(pre|post)[A-Z][A-Za-z]*|script|shellHook)$")
  (#set! injection.language "bash")
  (#set! injection.combined))

; HCL: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A
; heredoc whose tag names a language holds that language.

((heredoc_template
  (template_literal) @injection.content
  (heredoc_identifier) @_tag)
  (#any-of? @_tag "JSON" "json")
  (#set! injection.language "json"))

((heredoc_template
  (template_literal) @injection.content
  (heredoc_identifier) @_tag)
  (#any-of? @_tag "YAML" "yaml" "YML" "yml")
  (#set! injection.language "yaml"))

((heredoc_template
  (template_literal) @injection.content
  (heredoc_identifier) @_tag)
  (#any-of? @_tag "BASH" "bash" "SH" "sh" "SHELL" "shell")
  (#set! injection.language "bash"))

((heredoc_template
  (template_literal) @injection.content
  (heredoc_identifier) @_tag)
  (#any-of? @_tag "PYTHON" "python" "PY" "py")
  (#set! injection.language "python"))

((heredoc_template
  (template_literal) @injection.content
  (heredoc_identifier) @_tag)
  (#any-of? @_tag "SQL" "sql")
  (#set! injection.language "sql"))

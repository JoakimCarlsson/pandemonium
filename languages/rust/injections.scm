; Rust: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language.

((line_comment
  (doc_comment) @injection.content)
  (#set! injection.language "markdown")
  (#set! injection.combined))

((block_comment
  (doc_comment) @injection.content)
  (#set! injection.language "markdown")
  (#set! injection.combined))

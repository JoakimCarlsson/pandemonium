; Markdown: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; text of each block is inline markdown, and a fenced block holds the language
; its info string names.

(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content)

((minus_metadata) @injection.content
  (#set! injection.language "yaml"))

((plus_metadata) @injection.content
  (#set! injection.language "toml"))

((inline) @injection.content
  (#set! injection.language "markdown-inline"))

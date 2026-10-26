; MDX: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A block
; that opens with a tag is JSX, and a paragraph that starts with import or
; export is the module's own code; the text of every other block is inline
; markdown, and a fenced block holds the language its info string names.

(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content)

((minus_metadata) @injection.content
  (#set! injection.language "yaml"))

((plus_metadata) @injection.content
  (#set! injection.language "toml"))

((html_block) @injection.content
  (#set! injection.language "tsx"))

((paragraph
  (inline) @injection.content)
  (#match? @injection.content "^(import|export)[ \t]")
  (#set! injection.language "tsx"))

((inline) @injection.content
  (#not-match? @injection.content "^(import|export)[ \t]")
  (#set! injection.language "markdown-inline"))

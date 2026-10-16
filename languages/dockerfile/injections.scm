; Dockerfile: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; command of a RUN, CMD or other shell line, and the heredoc a RUN feeds, are
; each a shell script.

((shell_command) @injection.content
  (#set! injection.language "bash"))

((run_instruction
  (heredoc_block) @injection.content)
  (#set! injection.language "bash"))

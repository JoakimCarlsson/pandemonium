; Make: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. Each
; line of a recipe, and the command of a shell assignment, is Bash.

((recipe_line
  (shell_text) @injection.content)
  (#set! injection.language "bash"))

((shell_command) @injection.content
  (#set! injection.language "bash"))

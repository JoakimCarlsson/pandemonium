; JavaScript: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A
; template tagged with the name of a language holds that language.

(call_expression
  function: (identifier) @injection.language
  arguments: (template_string
    (string_fragment) @injection.content)
  (#any-of? @injection.language "markdown" "md" "json" "toml" "graphql" "gql")
  (#set! injection.combined))

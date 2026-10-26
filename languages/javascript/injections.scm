; JavaScript: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A
; template tagged with the name of a language holds that language, and one
; of a CSS-in-JS library holds CSS.

(call_expression
  function: (identifier) @injection.language
  arguments: (template_string
    (string_fragment) @injection.content)
  (#any-of? @injection.language "markdown" "md" "json" "toml" "graphql" "gql")
  (#set! injection.combined))

; Styles written in a template of a CSS-in-JS library hold CSS: styled.div`...`,
; styled(Button)`...`, either of those after .attrs(...) or .withConfig(...),
; and css`...`, keyframes`...`, createGlobalStyle`...` and injectGlobal`...`.
; The ${} parts are left to the language the file is written in.

(call_expression
  function: (member_expression
    object: (identifier) @_styled)
  arguments: (template_string
    (string_fragment) @injection.content)
  (#eq? @_styled "styled")
  (#set! injection.language "css")
  (#set! injection.combined))

(call_expression
  function: (call_expression
    function: (identifier) @_styled)
  arguments: (template_string
    (string_fragment) @injection.content)
  (#eq? @_styled "styled")
  (#set! injection.language "css")
  (#set! injection.combined))

(call_expression
  function: (call_expression
    function: (member_expression
      object: [
        (identifier) @_styled
        (member_expression
          object: (identifier) @_styled)
        (call_expression
          function: (identifier) @_styled)
      ]
      property: (property_identifier) @_configure))
  arguments: (template_string
    (string_fragment) @injection.content)
  (#eq? @_styled "styled")
  (#any-of? @_configure "attrs" "withConfig")
  (#set! injection.language "css")
  (#set! injection.combined))

(call_expression
  function: (identifier) @_tag
  arguments: (template_string
    (string_fragment) @injection.content)
  (#any-of? @_tag "css" "keyframes" "createGlobalStyle" "injectGlobal")
  (#set! injection.language "css")
  (#set! injection.combined))

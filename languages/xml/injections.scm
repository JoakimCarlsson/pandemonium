; XML: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A style
; element, as in SVG, holds CSS, and a script element JavaScript.

((element
  (STag
    (Name) @_name)
  (content) @injection.content)
  (#eq? @_name "style")
  (#set! injection.language "css"))

((element
  (STag
    (Name) @_name)
  (content) @injection.content)
  (#eq? @_name "script")
  (#set! injection.language "javascript"))

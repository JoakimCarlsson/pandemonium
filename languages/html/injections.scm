; HTML: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A style
; element holds CSS, and so does a style attribute whose last declaration ends
; in a semicolon, the only way the CSS grammar reads declarations outside a
; rule. A script element holds JavaScript, or JSON when its type says so.

((style_element
  (raw_text) @injection.content)
  (#set! injection.language "css"))

((attribute
  (attribute_name) @_attribute
  (quoted_attribute_value
    (attribute_value) @injection.content))
  (#eq? @_attribute "style")
  (#match? @injection.content ";\\s*$")
  (#set! injection.language "css"))

((script_element
  (start_tag
    (attribute
      (attribute_name) @_attribute
      (quoted_attribute_value
        (attribute_value) @_type)))
  (raw_text) @injection.content)
  (#eq? @_attribute "type")
  (#match? @_type "^(importmap|speculationrules|application/(ld\\+)?json)$")
  (#set! injection.language "json"))

((script_element
  (start_tag) @_start
  (raw_text) @injection.content)
  (#not-match? @_start "\\stype\\s*=")
  (#set! injection.language "javascript"))

((script_element
  (start_tag
    (attribute
      (attribute_name) @_attribute
      (quoted_attribute_value
        (attribute_value) @_type)))
  (raw_text) @injection.content)
  (#eq? @_attribute "type")
  (#match? @_type "^(module|(text|application)/(javascript|ecmascript))$")
  (#set! injection.language "javascript"))

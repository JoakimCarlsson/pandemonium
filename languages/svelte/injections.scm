; Svelte: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. A script
; element holds JavaScript, or TypeScript when its lang says so, and a style
; element CSS, or SCSS when its lang says so. The expressions between braces in
; the markup are JavaScript.

((script_element
  (start_tag
    (attribute
      (attribute_name) @_attribute
      (quoted_attribute_value
        (attribute_value) @_lang)))
  (raw_text) @injection.content)
  (#eq? @_attribute "lang")
  (#any-of? @_lang "ts" "typescript")
  (#set! injection.language "typescript"))

((script_element
  (start_tag) @_start
  (raw_text) @injection.content)
  (#not-match? @_start "\\slang\\s*=\\s*[\"']?(ts|typescript)")
  (#set! injection.language "javascript"))

((style_element
  (start_tag
    (attribute
      (attribute_name) @_attribute
      (quoted_attribute_value
        (attribute_value) @_lang)))
  (raw_text) @injection.content)
  (#eq? @_attribute "lang")
  (#eq? @_lang "scss")
  (#set! injection.language "scss"))

((style_element
  (start_tag) @_start
  (raw_text) @injection.content)
  (#not-match? @_start "\\slang\\s*=\\s*[\"']?scss")
  (#set! injection.language "css"))

((svelte_raw_text) @injection.content
  (#set! injection.language "javascript"))

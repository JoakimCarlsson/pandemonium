; JSON: the places that carry a lens. @lens.<kind> is the whole place and @name
; what it is called. The lenses are declared for `package.json` alone.

; Each script of the `scripts` object.
(document
  (object
    (pair
      key: (string
        (string_content) @_section)
      value: (object
        (pair
          key: (string
            (string_content) @name)) @lens.script)))
  (#eq? @_section "scripts"))

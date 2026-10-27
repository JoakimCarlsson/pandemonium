; Make: the places that carry a lens. @lens.<kind> is the whole place and @name
; what it is called. Special targets and pattern rules are left out.

(rule
  (targets
    (word) @name)
  (#not-match? @name "^[.%]")) @lens.target

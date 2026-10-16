; Make: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. Rules are listed by their
; targets, leaving out the special targets that start with a dot.

((rule
  (targets) @name) @item
  (#not-match? @name "^\\."))

(define_directive
  "define" @context
  name: (word) @name) @item

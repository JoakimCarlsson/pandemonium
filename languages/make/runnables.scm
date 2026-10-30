; Make: the targets of a Makefile are the places that can be run. Each target
; is tagged make-target and its name is $PM_CUSTOM_target; names that only
; stand for a pattern, a variable or make's own are left out.

(rule
  (targets
    (word) @run @target
    (#match? @target "^[A-Za-z0-9_/][A-Za-z0-9_./-]*$")
    (#set! tag make-target)))

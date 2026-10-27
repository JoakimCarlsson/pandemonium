; Dockerfile: the places that carry a lens. @lens.<kind> is the whole place and
; @name what it is called.

(from_instruction) @lens.build

(from_instruction
  (image_alias) @name) @lens.stage

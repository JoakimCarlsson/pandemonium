; XML: tokens that pair up, @open with the @close of the same match. The start
; and end tags of an element pair up by their names.

(element
  (STag
    (Name) @open)
  (ETag
    (Name) @close))

(EmptyElemTag
  "<" @open
  "/>" @close)

("(" @open ")" @close)
("[" @open "]" @close)

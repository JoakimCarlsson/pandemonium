; XML: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. Elements with an id or a
; name are listed by it, and the declarations of a DTD by what they declare.

(element
  (STag
    (Name) @context
    (Attribute
      (Name) @_attribute
      (AttValue) @name))
  (#any-of? @_attribute "id" "xml:id" "name")) @item

(element
  (EmptyElemTag
    (Name) @context
    (Attribute
      (Name) @_attribute
      (AttValue) @name))
  (#any-of? @_attribute "id" "xml:id" "name")) @item

(elementdecl
  "ELEMENT" @context
  (Name) @name) @item

(AttlistDecl
  "ATTLIST" @context
  (Name) @name) @item

(GEDecl
  "ENTITY" @context
  (Name) @name) @item

(PEDecl
  "ENTITY" @context
  (Name) @name) @item

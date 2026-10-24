; XML: what colour each node is. The styles and scripts inside a document are
; their own languages', injected. Where two patterns capture the same node, the
; one written first wins.

(Comment) @comment

(XMLDecl
  "xml" @keyword)
(XMLDecl
  [
    "version"
    "encoding"
    "standalone"
  ] @attribute)
(XMLDecl
  (EncName) @string)
(XMLDecl
  (VersionNum) @number)
(XMLDecl
  [
    "yes"
    "no"
  ] @boolean)

(PI
  (PITarget) @keyword)
(XmlModelPI
  "xml-model" @keyword)
(StyleSheetPI
  "xml-stylesheet" @keyword)
(PseudoAtt
  (Name) @attribute)
(PseudoAtt
  (PseudoAttValue) @string)

(doctypedecl
  "DOCTYPE" @keyword)
(doctypedecl
  (Name) @type)

(elementdecl
  "ELEMENT" @keyword
  (Name) @tag)
(contentspec
  (_
    (Name) @attribute))
"#PCDATA" @type.builtin
[
  "EMPTY"
  "ANY"
] @keyword

(GEDecl
  "ENTITY" @keyword
  (Name) @constant)
(PEDecl
  "ENTITY" @keyword
  (Name) @constant)
(EntityValue) @string
(NDataDecl
  "NDATA" @keyword
  (Name) @label)
(NotationDecl
  "NOTATION" @keyword
  (Name) @label)

(AttlistDecl
  "ATTLIST" @keyword
  (Name) @tag)
(AttDef
  (Name) @attribute)
(AttDef
  (Enumeration
    (Nmtoken) @string))
(DefaultDecl
  (AttValue) @string)
[
  (StringType)
  (TokenizedType)
] @type.builtin
(NotationType
  "NOTATION" @type.builtin)
[
  "#REQUIRED"
  "#IMPLIED"
  "#FIXED"
] @keyword

[
  "PUBLIC"
  "SYSTEM"
] @keyword
(PubidLiteral) @string
(SystemLiteral
  (URI) @markup.link)

((EntityRef) @constant.builtin
  (#any-of? @constant.builtin "&amp;" "&lt;" "&gt;" "&quot;" "&apos;"))
(EntityRef) @constant
(CharRef) @string.escape
(PEReference) @constant

(STag
  (Name) @tag)
(ETag
  (Name) @tag)
(EmptyElemTag
  (Name) @tag)

(Attribute
  (Name) @attribute)
(Attribute
  (AttValue) @string)

(CDSect
  (CDStart) @punctuation.special
  (CData) @markup.raw
  "]]>" @punctuation.special)

[
  "<?"
  "?>"
  "<"
  ">"
  "</"
  "/>"
  "<!"
] @punctuation.bracket

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  "*"
  "?"
  "+"
  "%"
  ","
  "|"
  "="
] @operator

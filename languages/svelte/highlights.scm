; Svelte: what colour each node is. The scripts, styles and expressions of a
; component are their own languages', injected. Where two patterns capture the
; same node, the one written first wins.

(comment) @comment

(doctype) @keyword
"<!" @punctuation.bracket

[
  "if"
  "else if"
  "else"
  "each"
  "await"
  "then"
  "catch"
  "key"
  "snippet"
  "as"
  "html"
  "const"
  "debug"
  "render"
] @keyword

(snippet_name) @function

((tag_name) @type
  (#match? @type "^[A-Z]"))
(tag_name) @tag
(erroneous_end_tag_name) @tag

((attribute_name) @keyword
  (#match? @keyword "^(on|bind|use|transition|in|out|animate|class|style|let):"))
(attribute_name) @attribute

(attribute
  (attribute_name) @_name
  (quoted_attribute_value
    (attribute_value) @markup.link)
  (#any-of? @_name "href" "src"))

(quoted_attribute_value) @string
(attribute_value) @string

(entity) @string.escape

[
  "{"
  "}"
] @punctuation.special

[
  "#"
  ":"
  "/"
  "@"
] @punctuation.special

[
  "<"
  ">"
  "</"
  "/>"
] @punctuation.bracket

"=" @operator

; Cargo.toml: the crates it depends on. @name is the crate, @version the version
; asked for. A crate that comes from a git repository, another registry or a
; path, or is renamed, is left out.

; serde = "1.0"
(table
  [(bare_key) (dotted_key)] @_table
  (pair
    (bare_key) @name
    (string) @version)
  (#match? @_table "dependencies$"))

; toml = { version = "0.8", features = ["x"] }
(table
  [(bare_key) (dotted_key)] @_table
  (pair
    (bare_key) @name
    (inline_table
      (pair
        (bare_key) @_key
        (string) @version)))
  (#match? @_table "dependencies$")
  (#eq? @_key "version"))

; [dependencies.tokio]
; version = "1"
(table
  (dotted_key
    (bare_key) @_table
    .
    (bare_key) @name
    .)
  (pair
    (bare_key) @_key
    (string) @version)
  (#match? @_table "dependencies$")
  (#eq? @_key "version"))

(pair
  (inline_table
    (pair
      (bare_key) @_other)
    (#any-of? @_other "git" "registry" "path" "package"))) @skip

(table
  (pair
    (bare_key) @_other)
  (#any-of? @_other "git" "registry" "path" "package")) @skip

; TOML: the dependencies a Cargo.toml lists, written as `name = "version"` or
; as `name = { version = "version" }`, each with its name and version.

(table
  (bare_key) @_section
  (pair
    (bare_key) @name
    (string) @version)
  (#match? @_section "^(dependencies|dev-dependencies|build-dependencies)$"))

(table
  (bare_key) @_section
  (pair
    (bare_key) @name
    (inline_table
      (pair
        (bare_key) @_key
        (string) @version)))
  (#match? @_section "^(dependencies|dev-dependencies|build-dependencies)$")
  (#eq? @_key "version"))

(table
  (dotted_key
    (bare_key) @_workspace
    (bare_key) @_section)
  (pair
    (bare_key) @name
    (string) @version)
  (#eq? @_workspace "workspace")
  (#eq? @_section "dependencies"))

(table
  (dotted_key
    (bare_key) @_workspace
    (bare_key) @_section)
  (pair
    (bare_key) @name
    (inline_table
      (pair
        (bare_key) @_key
        (string) @version)))
  (#eq? @_workspace "workspace")
  (#eq? @_section "dependencies")
  (#eq? @_key "version"))

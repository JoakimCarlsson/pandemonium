; package.json: the packages it depends on. @name is the package, @version the
; version asked for. A package that comes from a path, an address or a tag is
; left out because its version is not a number.

(document
  (object
    (pair
      key: (string
        (string_content) @_section)
      value: (object
        (pair
          key: (string
            (string_content) @name)
          value: (string
            (string_content) @version))))
    (#match? @_section "^(dependencies|devDependencies|peerDependencies|optionalDependencies)$")))

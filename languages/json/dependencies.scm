; JSON: the dependencies a package.json lists, each with its name and the
; version it asks for.

(document
  (object
    (pair
      key: (string (string_content) @_section)
      value: (object
        (pair
          key: (string (string_content) @name)
          value: (string (string_content) @version)))))
  (#match? @_section "^(dependencies|devDependencies|peerDependencies|optionalDependencies)$"))

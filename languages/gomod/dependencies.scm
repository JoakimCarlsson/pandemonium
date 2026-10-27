; go.mod: the modules it requires. @name is the module, @version the version
; asked for; a module marked `// indirect` is left out.

(require_spec
  (module_path) @name
  (version) @version)

((require_spec) @skip
  (#match? @skip "//[ \t]*indirect"))

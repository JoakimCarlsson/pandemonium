; Sample: values that are hidden when the editor is asked to redact private ones.

(assignment
  left: (identifier) @_name
  right: (string) @redact
  (#match? @_name "(?i)secret|token|password|key"))

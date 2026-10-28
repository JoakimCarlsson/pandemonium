; TOML: the scripts a pyproject.toml installs, from [project.scripts] and
; [tool.poetry.scripts], are the places that can be run. Each one is tagged
; pyproject-script and its name is $PM_CUSTOM_script.

(
  (table
    (dotted_key
      (bare_key) @_section
      (bare_key) @_name)
    (pair
      (bare_key) @script) @run)
  (#eq? @_section "project")
  (#eq? @_name "scripts")
  (#set! tag pyproject-script))

(
  (table
    (dotted_key
      (dotted_key
        (bare_key) @_tool
        (bare_key) @_section)
      (bare_key) @_name)
    (pair
      (bare_key) @script) @run)
  (#eq? @_tool "tool")
  (#eq? @_section "poetry")
  (#eq? @_name "scripts")
  (#set! tag pyproject-script))

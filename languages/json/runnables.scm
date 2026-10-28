; JSON: the scripts of a package.json are the places that can be run. Each
; one is tagged package-json-script and its name is $PM_CUSTOM_script.

(
  (document
    (object
      (pair
        key: (string (string_content) @_key)
        value: (object
          (pair
            key: (string (string_content) @script)) @run))))
  (#eq? @_key "scripts")
  (#set! tag package-json-script))

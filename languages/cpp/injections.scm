; C++: stretches written in another language. @injection.content is the
; stretch, and the language is set or captured as @injection.language. The
; body of a macro is C++ itself, and a raw string is in the language its
; delimiter names, as in R"sql(...)sql".

((preproc_arg) @injection.content
  (#set! injection.language "cpp"))

(raw_string_literal
  delimiter: (raw_string_delimiter) @injection.language
  (raw_string_content) @injection.content)

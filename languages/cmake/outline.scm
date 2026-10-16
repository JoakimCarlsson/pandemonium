; CMake: the items an outline lists. @item is the whole item, @name what it is
; called and @context the words that say what it is. Functions, macros, the
; project and the targets it builds are listed.

(function_def
  (function_command
    (function) @context
    (argument_list
      .
      (argument) @name))) @item

(macro_def
  (macro_command
    (macro) @context
    (argument_list
      .
      (argument) @name))) @item

((normal_command
  (identifier) @context
  (argument_list
    .
    (argument) @name)) @item
  (#match? @context "^(?i)(project|add_executable|add_library|add_custom_target|add_test)$"))

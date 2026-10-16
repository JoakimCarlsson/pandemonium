; CMake: what colour each node is. Where two patterns capture the same node,
; the one written first wins. Command names are read whatever their case.

[
  (bracket_comment)
  (line_comment)
] @comment

(escape_sequence) @string.escape

[
  "$"
  "{"
  "}"
] @punctuation.special
[
  "ENV"
  "CACHE"
] @namespace
(variable) @variable

[
  (quoted_argument)
  (bracket_argument)
] @string

[
  (function)
  (endfunction)
  (macro)
  (endmacro)
  (if)
  (elseif)
  (else)
  (endif)
  (foreach)
  (endforeach)
  (while)
  (endwhile)
  (block)
  (endblock)
] @keyword

(normal_command
  (identifier) @keyword
  (#match? @keyword "^(?i)(break|continue|return)$"))

(function_command
  (argument_list
    .
    (argument) @function
    (argument)* @variable.parameter))
(macro_command
  (argument_list
    .
    (argument) @function.macro
    (argument)* @variable.parameter))

(normal_command
  (identifier) @function.builtin
  (#match? @function.builtin "^(?i)(cmake_host_system_information|cmake_language|cmake_minimum_required|cmake_parse_arguments|cmake_path|cmake_policy|configure_file|execute_process|file|find_file|find_library|find_package|find_path|find_program|get_cmake_property|get_directory_property|get_filename_component|get_property|include|include_guard|list|mark_as_advanced|math|message|option|separate_arguments|set|set_directory_properties|set_property|site_name|string|unset|variable_watch|add_compile_definitions|add_compile_options|add_custom_command|add_custom_target|add_definitions|add_dependencies|add_executable|add_library|add_link_options|add_subdirectory|add_test|aux_source_directory|build_command|create_test_sourcelist|define_property|enable_language|enable_testing|export|fltk_wrap_ui|get_source_file_property|get_target_property|get_test_property|include_directories|include_external_msproject|include_regular_expression|install|link_directories|link_libraries|load_cache|project|remove_definitions|set_source_files_properties|set_target_properties|set_tests_properties|source_group|target_compile_definitions|target_compile_features|target_compile_options|target_include_directories|target_link_directories|target_link_libraries|target_link_options|target_precompile_headers|target_sources|try_compile|try_run|ctest_build|ctest_configure|ctest_coverage|ctest_empty_binary_directory|ctest_memcheck|ctest_read_custom_files|ctest_run_script|ctest_sleep|ctest_start|ctest_submit|ctest_test|ctest_update|ctest_upload|fetchcontent_declare|fetchcontent_makeavailable)$"))
(normal_command
  (identifier) @function)

((normal_command
  (identifier) @_command
  (argument_list
    .
    (argument) @variable))
  (#match? @_command "^(?i)(set|unset|option|list|string)$"))

((argument
  (unquoted_argument) @constant.builtin)
  (#match? @constant.builtin "^(?i)(on|off|yes|no|true|false|y|n|ignore|notfound|.*-notfound)$"))
([
  (if_command
    (argument_list
      (argument
        (unquoted_argument) @keyword)))
  (elseif_command
    (argument_list
      (argument
        (unquoted_argument) @keyword)))
  (while_command
    (argument_list
      (argument
        (unquoted_argument) @keyword)))
]
  (#any-of? @keyword
    "NOT" "AND" "OR" "COMMAND" "POLICY" "TARGET" "TEST" "DEFINED" "IN_LIST" "EXISTS" "IS_NEWER_THAN"
    "IS_DIRECTORY" "IS_SYMLINK" "IS_ABSOLUTE" "MATCHES" "LESS" "GREATER" "EQUAL" "LESS_EQUAL"
    "GREATER_EQUAL" "STRLESS" "STRGREATER" "STREQUAL" "STRLESS_EQUAL" "STRGREATER_EQUAL"
    "VERSION_LESS" "VERSION_GREATER" "VERSION_EQUAL" "VERSION_LESS_EQUAL" "VERSION_GREATER_EQUAL"))
((argument
  (unquoted_argument) @constant)
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))

[
  "("
  ")"
] @punctuation.bracket

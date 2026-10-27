/// A go.sum file: one line for each module version, with the module's path, the
/// version and its hash.
module.exports = grammar({
  name: "gosum",

  rules: {
    source_file: ($) => repeat($.entry),

    entry: ($) =>
      seq(
        field("module", $.module_path),
        field("version", $.version),
        field("hash", $.hash),
      ),

    module_path: (_) => /[^\s]+/,

    version: (_) => /v[^\s]+/,

    hash: (_) => /h1:[^\s]+/,
  },
});

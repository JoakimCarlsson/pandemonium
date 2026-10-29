//! The variables a language folder or an extension may add to the environment of a program it names.

use std::collections::BTreeMap;

/// The names, whole, that change what a shell or a program runs before it does what it was asked.
const REFUSED: [&str; 13] = [
    "PATH",
    "IFS",
    "ENV",
    "BASH_ENV",
    "BASHOPTS",
    "SHELLOPTS",
    "PROMPT_COMMAND",
    "PS4",
    "GCONV_PATH",
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "HOME",
];

/// The beginnings of names of whole families that load code or configuration of the writer's choosing.
const REFUSED_PREFIXES: [&str; 8] = [
    "LD_",
    "DYLD_",
    "PYTHON",
    "PERL",
    "RUBY",
    "NODE_",
    "NPM_CONFIG_",
    "GIT_",
];

/// `env` without the variables that make a program load code or run a script of the writer's choosing, or that move its search path or home: the names and families above. What a toolchain needs, such as `GOBIN`, `VIRTUAL_ENV` or `RUSTUP_TOOLCHAIN`, stays; its folders go in its `path`, which is the only way to change the search path.
pub fn safe_env(env: BTreeMap<String, String>) -> BTreeMap<String, String> {
    env.into_iter()
        .filter(|(name, _)| {
            let upper = name.to_ascii_uppercase();
            !REFUSED.contains(&upper.as_str())
                && !REFUSED_PREFIXES
                    .iter()
                    .any(|prefix| upper.starts_with(prefix))
        })
        .collect()
}

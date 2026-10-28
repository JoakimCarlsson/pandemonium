//! The variables a language folder or an extension may add to the environment of a program it names.

use std::collections::BTreeMap;

/// `env` without the variables that make a program load code or run a script of the writer's choosing: `LD_*`, `DYLD_*`, `NODE_OPTIONS`, `BASH_ENV`, `ENV`, `PYTHONSTARTUP`, `PYTHONPATH` and `PERL5OPT`.
pub fn safe_env(env: BTreeMap<String, String>) -> BTreeMap<String, String> {
    const REFUSED: [&str; 6] = [
        "NODE_OPTIONS",
        "BASH_ENV",
        "ENV",
        "PYTHONSTARTUP",
        "PYTHONPATH",
        "PERL5OPT",
    ];
    env.into_iter()
        .filter(|(name, _)| {
            let upper = name.to_ascii_uppercase();
            !upper.starts_with("LD_")
                && !upper.starts_with("DYLD_")
                && !REFUSED.contains(&upper.as_str())
        })
        .collect()
}

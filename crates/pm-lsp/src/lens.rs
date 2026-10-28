//! The links a language shows at the end of the lines it marks as runnable, in place of a run button: each is a task of its `tasks.json` that says it is a lens.

use serde::{Deserialize, Serialize};

use crate::inherit::language_file;

/// A link at the end of a runnable line that runs one task of the language.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunLens {
    /// The tags of the places it is shown on.
    pub tags: Vec<String>,
    /// What it says, and what the task is called, which running it names the task by.
    pub label: String,
}

/// The lenses of the installed language `language`: the templates of its `tasks.json` that are lenses, each shown on the places carrying any of their tags.
pub fn run_lenses_of(language: &str) -> Vec<RunLens> {
    let Some(text) =
        language_file(language, "tasks.json").and_then(|file| std::fs::read_to_string(file).ok())
    else {
        return Vec::new();
    };
    pm_task::parse_templates(&text)
        .unwrap_or_default()
        .into_iter()
        .filter(|template| template.lens)
        .map(|template| RunLens {
            tags: template.tags,
            label: template.label,
        })
        .collect()
}

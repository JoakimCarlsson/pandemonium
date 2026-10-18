//! What the reader set up for the work a server does on its machine: how it runs git, what new sessions start with, the shell it opens and how each language is handled.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How often a server fetches as it comes, in minutes.
pub const FETCH_MINUTES: u32 = 15;

/// How many days a server keeps the states of a file as they come.
pub const HISTORY_DAYS: u32 = 30;

/// How many megabytes of kept states a server holds for one project as they come.
pub const HISTORY_MEGABYTES: u32 = 256;

/// How a server runs git for the projects on its machine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitPreferences {
    /// Whether it fetches every remote of each project by itself.
    pub auto_fetch: bool,
    /// How often it does, in minutes.
    pub fetch_minutes: u32,
    /// What a new session's branch is called, with `{title}` standing for its title; empty leaves the worktree on no branch.
    pub branch_template: String,
    /// The folder worktrees are made in, one folder per project inside it; empty for a folder inside each project.
    pub worktree_root: String,
}

impl Default for GitPreferences {
    /// Git as a server runs it as it comes: fetching every quarter of an hour, worktrees on no branch inside their project.
    fn default() -> Self {
        Self {
            auto_fetch: true,
            fetch_minutes: FETCH_MINUTES,
            branch_template: String::new(),
            worktree_root: String::new(),
        }
    }
}

/// How long and how much a server keeps of the states a file was saved in.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryPreferences {
    /// Whether it keeps a state of a file each time the file is saved.
    pub enabled: bool,
    /// How many days a state is kept; older ones are removed.
    pub days: u32,
    /// How many megabytes the states of one project may take up; the oldest are removed past it.
    pub megabytes: u32,
}

impl Default for HistoryPreferences {
    /// Local history as it comes: kept, for a month, up to a quarter of a gigabyte per project.
    fn default() -> Self {
        Self {
            enabled: true,
            days: HISTORY_DAYS,
            megabytes: HISTORY_MEGABYTES,
        }
    }
}

/// What a new session starts with.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPreferences {
    /// The agent new work is given to, by id, or the first one listed when nothing.
    pub default_agent: Option<String>,
    /// The mode an agent starts in, by the id agents give it, such as `acceptEdits` or `plan`, or the agent's own when nothing.
    pub permission_mode: Option<String>,
}

/// The shell a server opens for the terminal dock.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellPreferences {
    /// The program, or the machine's own login shell when empty.
    pub program: String,
    /// What it is started with, written as on a command line.
    pub arguments: String,
}

/// How the files of one language are edited and which language server reads them; each thing not set is as the editor or the language says.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguagePreferences {
    /// How many columns a level of indentation is.
    pub tab_size: Option<usize>,
    /// Whether a level of indentation is a tab.
    pub hard_tabs: Option<bool>,
    /// Whether saving formats the file.
    pub format_on_save: Option<bool>,
    /// Whether no language server reads its files.
    pub server_off: bool,
    /// The command started as its language server in place of the one it comes with, written as on a command line; empty for the one it comes with.
    pub server_command: String,
}

impl LanguagePreferences {
    /// Whether nothing is set, so the language is as it comes.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// What the reader set up for the work a server does on its machine.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSettings {
    /// How it runs git.
    pub git: GitPreferences,
    /// How long it keeps the states a file was saved in.
    pub history: HistoryPreferences,
    /// What new sessions start with.
    pub agents: AgentPreferences,
    /// The shell it opens.
    pub shell: ShellPreferences,
    /// How each language is handled, by the language's id; a language not named is as it comes.
    pub languages: BTreeMap<String, LanguagePreferences>,
}

impl MachineSettings {
    /// How the language `id` is handled.
    pub fn language(&self, id: &str) -> LanguagePreferences {
        self.languages.get(id).cloned().unwrap_or_default()
    }

    /// The settings with the language `id` handled as `preferences`, leaving it out when that is as it comes.
    pub fn set_language(&mut self, id: &str, preferences: LanguagePreferences) {
        if preferences.is_default() {
            self.languages.remove(id);
        } else {
            self.languages.insert(id.to_owned(), preferences);
        }
    }
}

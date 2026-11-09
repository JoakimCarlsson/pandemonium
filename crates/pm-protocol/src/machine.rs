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
    /// The ignored files and folders of a project, as paths from its root separated by commas or lines, copied into every new session's worktree; the common ones (`.env` files, `target/`, `node_modules/`) by default, empty copies nothing.
    pub worktree_include: String,
    /// The layout of a self-hosted server's web addresses, by the server's host name, for the servers whose name does not give it away: `github`, `gitlab`, `bitbucket`, `gitea`, `azure`, `sourcehut` or `gitee`.
    pub hosts: BTreeMap<String, String>,
}

impl GitPreferences {
    /// The hosts and their layouts written as `host=layout`, separated by commas.
    pub fn hosts_text(&self) -> String {
        self.hosts
            .iter()
            .map(|(host, layout)| format!("{host}={layout}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Replaces the hosts with those `text` names, written as `host=layout` separated by commas; a part that is not written so is left out.
    pub fn set_hosts_text(&mut self, text: &str) {
        self.hosts = text
            .split([',', '\n'])
            .filter_map(|part| part.split_once('='))
            .map(|(host, layout)| (host.trim().to_lowercase(), layout.trim().to_lowercase()))
            .filter(|(host, layout)| !host.is_empty() && !layout.is_empty())
            .collect();
    }
}

impl Default for GitPreferences {
    /// Git as a server runs it as it comes: fetching every quarter of an hour, worktrees on no branch inside their project.
    fn default() -> Self {
        Self {
            auto_fetch: true,
            fetch_minutes: FETCH_MINUTES,
            branch_template: String::new(),
            worktree_root: String::new(),
            worktree_include:
                ".env, .env.*, target/, node_modules/, .build/, build/, dist/, .venv/".to_owned(),
            hosts: BTreeMap::new(),
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPreferences {
    /// The agent new work is given to, by id, or the first one listed when nothing.
    pub default_agent: Option<String>,
    /// The mode an agent starts in, by the id agents give it, such as `default`, `plan` or `yolo`, or the agent's own when nothing.
    pub permission_mode: Option<String>,
    /// The agents, by id, left out of the ones a new session can be started with; they still run the sessions they have and keep their accounts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// How hard the model thinks, by the harness's id such as `low`, `medium`, `high`, `xhigh` or `max`, or the harness's own choice when nothing; a model that does not take it thinks at its own default.
    pub effort: Option<String>,
    /// Whether agents start with what earlier conversations in the project remembered, and can save memories.
    pub memory: bool,
    /// Whether agents have the checkpoint and rewind tools.
    pub checkpoints: bool,
    /// How long the prompt cache lives, `5m` or `1h`, where the provider lets it be picked.
    pub cache: String,
    /// Whether the prompt cache is kept warm while a long tool runs or the conversation waits, where the provider can.
    pub cache_warming: bool,
    /// The model plan mode plans with, as `model` or `provider/model`, or the conversation's own model when empty.
    pub plan_model: String,
    /// Whether each plan an agent submits is also saved as markdown in the project.
    pub plan_autosave: bool,
}

/// The prompt cache lifetime a new conversation starts with.
pub const CACHE_SHORT: &str = "5m";

impl Default for AgentPreferences {
    /// No default agent or mode, memory, checkpoints and cache warming on, the short cache, planning with the conversation's model, and plans kept with the conversation only.
    fn default() -> Self {
        Self {
            default_agent: None,
            permission_mode: None,
            hidden: Vec::new(),
            effort: None,
            memory: true,
            checkpoints: true,
            cache: CACHE_SHORT.to_owned(),
            cache_warming: true,
            plan_model: String::new(),
            plan_autosave: false,
        }
    }
}

impl AgentPreferences {
    /// The conversation settings a new conversation starts with, by the harness's setting id and value; a choice saved with a conversation wins over them.
    pub fn conversation_defaults(&self) -> Vec<(String, String)> {
        let switch = |on: bool| if on { "on" } else { "off" }.to_owned();
        let mut defaults = vec![
            ("memory".to_owned(), switch(self.memory)),
            ("checkpoints".to_owned(), switch(self.checkpoints)),
            ("cache".to_owned(), self.cache.clone()),
            ("cache_warming".to_owned(), switch(self.cache_warming)),
            ("plan_autosave".to_owned(), switch(self.plan_autosave)),
        ];
        let planner = self.plan_model.trim();
        if !planner.is_empty() {
            defaults.push(("plan_model".to_owned(), planner.to_owned()));
        }
        if let Some(effort) = &self.effort {
            defaults.push(("effort".to_owned(), effort.clone()));
        }
        defaults
    }
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
    /// Whether saving first organizes the imports of the file, as its language server says.
    pub organize_imports_on_save: Option<bool>,
    /// Whether saving first applies every fix of the language server that is safe to apply at once.
    pub fix_all_on_save: Option<bool>,
    /// Whether no language server reads its files.
    pub server_off: bool,
    /// The command started as its language server in place of the one it comes with, written as on a command line; empty for the one it comes with.
    pub server_command: String,
    /// Exactly the language servers that read its files, by id and in this order: the first is the language's server and the rest run beside it. `...` stands for the servers that run when nothing is chosen, and a server the language does not start by itself runs only when it is named. Empty leaves the language as it comes.
    pub servers: Vec<String>,
}

impl LanguagePreferences {
    /// Whether nothing is set, so the language is as it comes.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Which folders besides a project's own are attached to the workspace its language servers work on, so that one server reads the files of all of them.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspacePreferences {
    /// Whether the repositories kept inside a project and the submodules of its repository are attached by themselves.
    pub attach_repositories: bool,
    /// The folders attached to the projects named, each written as `project=folder` or as `folder` alone for every project; a project is named by its folder or by that folder's name, and a folder not given in full is taken from the root of the project.
    pub folders: Vec<String>,
}

impl WorkspacePreferences {
    /// The folders written as lines, one to a line.
    pub fn folders_text(&self) -> String {
        self.folders.join(", ")
    }

    /// Replaces the folders with those `text` names, separated by commas or lines; blanks are left out.
    pub fn set_folders_text(&mut self, text: &str) {
        self.folders = text
            .split([',', '\n'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
    }

    /// The folders written for the project at `root`, as paths: the ones written for every project and the ones written for this one, those not given in full taken from `root`.
    pub fn folders_for(&self, root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        self.folders
            .iter()
            .filter_map(|line| {
                let (project, folder) = match line.split_once('=') {
                    Some((project, folder)) => (Some(project.trim()), folder.trim()),
                    None => (None, line.as_str()),
                };
                let named = project.is_none_or(|project| {
                    std::path::Path::new(project) == root || name.as_deref() == Some(project)
                });
                (named && !folder.is_empty()).then(|| root.join(folder))
            })
            .collect()
    }

    /// Whether nothing is set, so projects have only their own folder.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// What the reader allowed one extension to do on the server's machine; nothing is allowed until it is granted.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtensionPreferences {
    /// The capabilities granted, by name: `process`, `download`, `read-project`.
    pub granted: Vec<String>,
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
    /// Which folders besides a project's own its language servers work on.
    pub workspace: WorkspacePreferences,
    /// What each extension is allowed to do, by the extension's folder name; an extension not named is allowed nothing.
    pub extensions: BTreeMap<String, ExtensionPreferences>,
    /// What is set for each language server, by the server's id, whichever language it reads; a server not named is as its language starts it.
    pub servers: BTreeMap<String, pm_lsp::ServerOverride>,
}

impl MachineSettings {
    /// How the language `id` is handled.
    pub fn language(&self, id: &str) -> LanguagePreferences {
        self.languages.get(id).cloned().unwrap_or_default()
    }

    /// The capabilities granted to the extension `id`, by name.
    pub fn granted(&self, id: &str) -> Vec<String> {
        self.extensions
            .get(id)
            .map(|held| held.granted.clone())
            .unwrap_or_default()
    }

    /// The settings with the extension `id` granted exactly `granted`, leaving it out when that is nothing.
    pub fn set_granted(&mut self, id: &str, granted: Vec<String>) {
        if granted.is_empty() {
            self.extensions.remove(id);
        } else {
            self.extensions
                .insert(id.to_owned(), ExtensionPreferences { granted });
        }
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

#[cfg(test)]
/// Tests for the machine's settings.
mod tests {
    use super::*;

    /// Settings saved before the conversation defaults existed read with memory, checkpoints and warming on, the short cache and no effort, and say so to the harness.
    #[test]
    fn older_agent_settings_keep_the_defaults() {
        let read: AgentPreferences = serde_json::from_str(r#"{"permission_mode":"plan"}"#).unwrap();
        assert_eq!(read.permission_mode.as_deref(), Some("plan"));
        assert_eq!(
            read.conversation_defaults(),
            [
                ("memory", "on"),
                ("checkpoints", "on"),
                ("cache", "5m"),
                ("cache_warming", "on"),
                ("plan_autosave", "off"),
            ]
            .map(|(id, value)| (id.to_owned(), value.to_owned()))
        );
        let chosen = AgentPreferences {
            effort: Some("high".to_owned()),
            memory: false,
            plan_model: " anthropic/claude-opus-5-5 ".to_owned(),
            ..read
        };
        let defaults = chosen.conversation_defaults();
        assert!(defaults.contains(&("memory".to_owned(), "off".to_owned())));
        assert!(defaults.contains(&("effort".to_owned(), "high".to_owned())));
        assert!(defaults.contains(&(
            "plan_model".to_owned(),
            "anthropic/claude-opus-5-5".to_owned()
        )));
    }
}

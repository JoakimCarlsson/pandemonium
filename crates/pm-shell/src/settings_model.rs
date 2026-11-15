//! What the settings window shows and edits: the settings themselves, what
//! the server told of the agents, tool servers, model servers, languages and
//! customizations, and where the window stands, such as the section open.

pub use pm_protocol::library::{Item, Kind, ToolSet};

use pm_config::WindowSettings;
use pm_editor::EditorSettings;
use pm_protocol::{ExtensionInfo, MachineSettings, McpServer};

use crate::settings_catalog::Setting;
use crate::settings_section::Section;

/// One agent that can be started, as the settings window lists it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentSetting {
    /// What the app keeps the agent under.
    pub id: String,
    /// What a row calls it.
    pub name: String,
    /// Whether it stands for a model server the reader declared.
    pub server: bool,
    /// How it is set up, such as which variable holds its key, when the harness says.
    pub hint: Option<String>,
}

impl AgentSetting {
    /// The agents the harness offers.
    pub fn shipped() -> Vec<Self> {
        pm_harness::agents()
            .iter()
            .map(|agent| Self {
                id: agent.id.to_owned(),
                name: agent.name.to_owned(),
                server: agent.server,
                hint: agent.setup_hint(),
            })
            .collect()
    }
}

/// What looking for a newer release on the channel followed found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Release {
    /// The lookup failed.
    Unknown,
    /// Nothing newer than the running build is out.
    Current,
    /// A newer release is out.
    Newer {
        /// What it is called, as the reader is told.
        name: String,
        /// The page it is published on.
        url: String,
    },
}

/// The version of the running build.
pub const RUNNING: &str = env!("CARGO_PKG_VERSION");

impl Release {
    /// What the server's answer of `version` published at `url` means for the running build.
    pub fn judged(version: Option<String>, url: String) -> Self {
        match version {
            Some(version) if !url.is_empty() && is_newer(&version, RUNNING) => Self::Newer {
                name: format!("Pandemonium {version}"),
                url,
            },
            Some(_) => Self::Current,
            None => Self::Unknown,
        }
    }
}

/// The numbers of `version` before any `-`, and whether it has no part after a `-`, which ranks a release over its pre-releases.
fn order(version: &str) -> (Vec<u64>, bool) {
    let version = version.trim_start_matches('v');
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    (
        core.split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect(),
        pre.is_none(),
    )
}

/// Whether `newest` is a later version than `running`.
fn is_newer(newest: &str, running: &str) -> bool {
    order(newest) > order(running)
}

/// The settings the window edits, held together so one setting can be copied from one set to another.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kept {
    /// How the editor looks and behaves.
    pub editor: EditorSettings,
    /// What the window keeps of its own.
    pub window: WindowSettings,
    /// What the server of each machine keeps.
    pub machine: MachineSettings,
}

impl Kept {
    /// The settings as they come.
    pub fn defaults() -> Self {
        Self::default()
    }
}

/// Where the settings window stands: the section open, what is searched and which list is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsPage {
    /// The section picked in the list down the left side.
    pub section: Section,
    /// What every setting is searched for; blank shows the section picked.
    pub find: String,
    /// The setting whose list of choices is open under it, when one is.
    pub picker: Option<Setting>,
    /// What the open list of choices is narrowed to.
    pub filter: String,
    /// Whether the sections inside Agents are folded away.
    pub folded: bool,
    /// What the keymap's actions are searched for.
    pub keymap_find: String,
    /// What the lists of customizations and tool servers are searched for.
    pub item_find: String,
}

/// Everything the settings window shows and edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsModel {
    /// How the editor looks and behaves.
    pub editor: EditorSettings,
    /// What the window keeps of its own.
    pub window: WindowSettings,
    /// What the server of each machine keeps.
    pub machine: MachineSettings,
    /// Whether the server on this machine told what it keeps, so changes to it may be sent.
    pub machine_loaded: bool,
    /// The families of the fonts installed on this machine.
    pub fonts: Vec<String>,
    /// The families among them whose letters are all as wide as each other.
    pub mono_fonts: Vec<String>,
    /// The language whose settings the languages page shows, by id; the first installed one when nothing.
    pub language: Option<String>,
    /// The agents that can be started.
    pub agents: Vec<AgentSetting>,
    /// The extensions of the server on this machine, languages among them.
    pub extensions: Vec<ExtensionInfo>,
    /// Whether the server told which extensions it has.
    pub extensions_loaded: bool,
    /// The tool servers every agent is started with.
    pub mcp_servers: Vec<McpServer>,
    /// Whether the server told which tool servers there are.
    pub mcp_loaded: bool,
    /// The model servers the reader declared.
    pub model_servers: Vec<pm_harness::ModelServer>,
    /// Whether the server told which model servers there are.
    pub model_servers_loaded: bool,
    /// The model slots.
    pub slots: Vec<pm_harness::SlotInfo>,
    /// The presets that set every slot at once.
    pub presets: Vec<pm_harness::PresetInfo>,
    /// Whether the server told the slots.
    pub slots_loaded: bool,
    /// The agent and project folder the customizations are read for, when there is a project.
    pub library_place: Option<(String, String)>,
    /// The customizations the agent is given in the project.
    pub library: Vec<Item>,
    /// The tools the agent is given, in sets.
    pub tool_sets: Vec<ToolSet>,
    /// The keys of the customizations, tool sets and tools turned off.
    pub withheld: Vec<String>,
    /// Whether the server told the customizations.
    pub library_loaded: bool,
    /// What looking for a newer release found, once it was looked for.
    pub release: Option<Release>,
    /// Where the window stands.
    pub page: SettingsPage,
    /// What the forms of the sections inside Agents hold.
    pub forms: crate::settings_forms::SettingsForms,
}

impl SettingsModel {
    /// The model with `editor` and `window` read from where the window keeps them, and the harness's agents.
    pub fn new(editor: EditorSettings, window: WindowSettings) -> Self {
        Self {
            editor,
            window,
            agents: AgentSetting::shipped(),
            ..Self::default()
        }
    }

    /// The settings the window edits, as they stand.
    pub fn kept(&self) -> Kept {
        Kept {
            editor: self.editor.clone(),
            window: self.window.clone(),
            machine: self.machine.clone(),
        }
    }

    /// Puts `kept` back as the settings the window edits.
    pub fn put_back(&mut self, kept: Kept) {
        self.editor = kept.editor;
        self.window = kept.window;
        self.machine = kept.machine;
    }

    /// The languages installed on the server on this machine, by id and name, in the order of their names.
    pub fn installed_languages(&self) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = self
            .languages()
            .into_iter()
            .filter(|extension| extension.installed)
            .map(|extension| (extension.id.clone(), extension.name.clone()))
            .collect();
        found.sort_by_key(|(_, name)| name.to_lowercase());
        found
    }

    /// The languages that can be installed or are, those installed first, each by name.
    pub fn languages(&self) -> Vec<&ExtensionInfo> {
        let mut listed: Vec<&ExtensionInfo> = self
            .extensions
            .iter()
            .filter(|extension| !extension.hidden && extension.kind != "theme")
            .collect();
        listed.sort_by_key(|extension| (!extension.installed, extension.name.to_lowercase()));
        listed
    }

    /// The language whose settings the languages page shows.
    pub fn settings_language(&self) -> Option<String> {
        let installed = || self.installed_languages().into_iter().map(|(id, _)| id);
        match &self.language {
            Some(id) if installed().any(|held| held == *id) => Some(id.clone()),
            _ => installed().next(),
        }
    }

    /// Whether `setting` shows: a language's settings only once a language is installed.
    pub fn shown(&self, setting: Setting) -> bool {
        match setting {
            Setting::LanguageTabSize
            | Setting::LanguageIndent
            | Setting::LanguageFormat
            | Setting::LanguageServer
            | Setting::LanguageServerCommand
            | Setting::LanguageServers
            | Setting::LanguageOrganize
            | Setting::LanguageFixAll => self.settings_language().is_some(),
            _ => true,
        }
    }

    /// Whether settings are being searched, which shows what was found in place of the section picked.
    pub fn searching(&self) -> bool {
        !self.page.find.trim().is_empty()
    }

    /// The section the window shows: what a search found, or the section picked.
    pub fn showing(&self) -> Section {
        if self.searching() {
            Section::Results
        } else {
            self.page.section
        }
    }

    /// Whether the agent `id` is offered for new sessions.
    pub fn agent_shown(&self, id: &str) -> bool {
        !self.machine.agents.hidden.iter().any(|held| held == id)
    }

    /// How many agents are offered for new sessions.
    pub fn shown_agents(&self) -> usize {
        self.agents
            .iter()
            .filter(|agent| self.agent_shown(&agent.id))
            .count()
    }

    /// The customizations of `kind` the agent is given.
    pub fn items_of(&self, kind: Kind) -> Vec<&Item> {
        self.library
            .iter()
            .filter(|item| item.kind == kind)
            .collect()
    }

    /// How many entries the section lists, for a page of customizations or tools.
    pub fn library_count(&self, section: Section) -> Option<usize> {
        if !self.library_loaded {
            return None;
        }
        match section {
            Section::Tools => Some(self.tool_sets.iter().map(|set| set.tools.len()).sum()),
            section => section.kind().map(|kind| self.items_of(kind).len()),
        }
    }

    /// Whether what `key` names is turned on.
    pub fn enabled(&self, key: &str) -> bool {
        !self.withheld.iter().any(|held| held == key)
    }
}

/// The ways a server can be reached, in the order the form offers them, with what they are called.
pub const REACHES: [(pm_config::Reach, &str); 5] = [
    (pm_config::Reach::Direct, "Address"),
    (pm_config::Reach::Ssh, "SSH"),
    (pm_config::Reach::Wsl, "WSL"),
    (pm_config::Reach::Container, "Dev container"),
    (pm_config::Reach::Tunnel, "Tunnel"),
];

/// How the known server `server` is reached, in words.
pub fn reach_of(server: &pm_config::KnownServer) -> String {
    let how = REACHES
        .iter()
        .find(|(reach, _)| *reach == server.reach)
        .map_or("Address", |(_, name)| name);
    if server.address.is_empty() {
        how.to_owned()
    } else {
        format!("{how} · {}", server.address)
    }
}

/// Where the model of `slot` is set, in words.
pub fn slot_source(slot: &pm_harness::SlotInfo) -> &'static str {
    match slot.source {
        pm_harness::SlotSource::Environment => "Environment",
        pm_harness::SlotSource::Project => "Project",
        pm_harness::SlotSource::User => "User",
        pm_harness::SlotSource::Main => "Conversation's model",
    }
}

#[cfg(test)]
/// Tests for how versions are ordered.
mod tests {
    use super::*;

    /// A later number, and a release over its own pre-release, count as newer.
    #[test]
    fn versions_order() {
        assert!(is_newer("0.15.0", "0.14.9"));
        assert!(is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(!is_newer("1.0.0-rc.1", "1.0.0"));
        assert!(!is_newer("0.14.0", "0.14.0"));
    }
}

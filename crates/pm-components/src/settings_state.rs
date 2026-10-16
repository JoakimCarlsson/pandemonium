//! What the settings window keeps while it is open, the settings it edits held together, and what is done once one of them changed.

use std::collections::HashMap;

use pm_config::WindowSettings;
use pm_editor::EditorSettings;
use pm_protocol::MachineSettings;
use pm_text::Field;

use crate::settings_catalog::{Keeper, Setting};
use crate::state::{Outgoing, WorkspaceState};

/// What the settings window keeps beside the settings themselves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsPage {
    /// The box every setting is searched with.
    pub find: Field,
    /// How far the list of sections is scrolled, in logical pixels.
    pub nav_scroll: f32,
    /// The boxes of the settings written as text, as they are being written.
    pub fields: HashMap<Setting, Field>,
    /// The setting whose list of choices is open under it, when one is.
    pub picker: Option<Setting>,
    /// What the open list of choices is narrowed to.
    pub filter: Field,
    /// The language whose settings the languages page shows, by id; the first installed one when nothing.
    pub language: Option<String>,
    /// The box the keymap's actions are searched with.
    pub keymap_find: Field,
    /// The families of the fonts installed on this machine, by name.
    pub fonts: Vec<String>,
    /// The families among them whose letters are all as wide as each other.
    pub mono_fonts: Vec<String>,
    /// The newest release on the channel followed, once asked: its version, when one was found, and its page.
    pub release: Option<(Option<String>, String)>,
    /// The form for adding or changing a server of the servers page, while it is open.
    pub server_form: Option<crate::settings_machines::MachineForm>,
}

/// The settings the window edits, held together so one setting can be copied from one set to another.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Kept {
    /// How the editor looks and behaves.
    pub(crate) editor: EditorSettings,
    /// What the window keeps of its own.
    pub(crate) window: WindowSettings,
    /// What the server of each machine keeps.
    pub(crate) machine: MachineSettings,
}

impl Kept {
    /// The settings as they come.
    pub(crate) fn defaults() -> Self {
        Self::default()
    }
}

impl WorkspaceState {
    /// The settings the window edits, as they stand.
    pub(crate) fn kept(&self) -> Kept {
        Kept {
            editor: self.editor.clone(),
            window: self.window.clone(),
            machine: self.machine.clone(),
        }
    }

    /// Puts `kept` back as the settings the window edits.
    pub(crate) fn put_back(&mut self, kept: Kept) {
        self.editor = kept.editor;
        self.window = kept.window;
        self.machine = kept.machine;
    }

    /// Puts `setting` to work once it changed: a setting the servers keep is sent to them, and the open files take what changed of their indentation.
    pub(crate) fn setting_changed(&mut self, setting: Setting) {
        if setting.keeper() == Keeper::Machine && self.machine_loaded {
            self.outbox
                .push(Outgoing::SaveMachineSettings(self.machine.clone()));
        }
        match setting {
            Setting::TabSize
            | Setting::HardTabs
            | Setting::LanguageTabSize
            | Setting::LanguageIndent => self.apply_indentation(),
            Setting::Scrollback => self.apply_scrollback(),
            Setting::Channel | Setting::CheckUpdates => {
                self.settings_page.release = None;
                if self.window.general.check_updates {
                    self.outbox.push(Outgoing::LatestRelease {
                        channel: self.window.general.channel,
                    });
                }
            }
            _ => {}
        }
    }

    /// Gives every setting written as text that has no box yet one holding its text.
    pub(crate) fn fill_fields(&mut self) {
        for setting in Setting::ALL.into_iter().filter(|setting| setting.is_text()) {
            if !self.settings_page.fields.contains_key(&setting) {
                let text = setting.text(self);
                self.settings_page.fields.insert(setting, Field::new(text));
            }
        }
    }

    /// The state with the window's own settings `settings`, read from where they were kept.
    pub fn with_window_settings(mut self, settings: WindowSettings) -> Self {
        self.window = settings;
        self.window_loaded = true;
        self.apply_scrollback();
        self
    }

    /// The state with the settings the server on this machine keeps, as it sent them, put to work.
    pub fn with_machine_settings(mut self, settings: MachineSettings) -> Self {
        self.machine = settings;
        self.machine_loaded = true;
        self.settings_page
            .fields
            .retain(|setting, _| setting.keeper() != Keeper::Machine);
        self.fill_fields();
        if let Some(agent) = self
            .machine
            .agents
            .default_agent
            .as_deref()
            .and_then(|id| self.agent_position(id))
            && self.sessions.is_empty()
        {
            self.agent = agent;
        }
        self.apply_indentation();
        self
    }

    /// The state knowing the font families installed on this machine: `all` of them, and the `mono` ones among them.
    pub fn with_fonts(mut self, all: Vec<String>, mono: Vec<String>) -> Self {
        self.settings_page.fonts = all;
        self.settings_page.mono_fonts = mono;
        self
    }

    /// The state knowing the newest release on the channel followed: its `version`, when one was found, and its page at `url`.
    pub fn with_release(mut self, version: Option<String>, url: String) -> Self {
        self.settings_page.release = Some((version, url));
        self
    }

    /// The language whose settings the languages page shows, by id, when any is installed.
    pub(crate) fn settings_language(&self) -> Option<String> {
        let installed = || self.installed_languages().into_iter().map(|(id, _)| id);
        match &self.settings_page.language {
            Some(id) if installed().any(|held| held == *id) => Some(id.clone()),
            _ => installed().next(),
        }
    }

    /// The languages installed on the server on this machine, by id and name, in the order of their names.
    pub(crate) fn installed_languages(&self) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = crate::extensions::pickable(&self.extensions)
            .filter(|extension| extension.installed && extension.kind != "theme")
            .map(|extension| (extension.id.clone(), extension.name.clone()))
            .collect();
        found.sort_by_key(|(_, name)| name.to_lowercase());
        found
    }
}

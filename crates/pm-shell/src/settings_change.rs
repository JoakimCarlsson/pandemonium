//! What the settings window asks to change, and how the model takes each
//! change in, answering with the requests the server must be sent.

use pm_protocol::Request;

use crate::settings_catalog::Setting;
use crate::settings_model::{Release, SettingsModel};
use crate::settings_section::Section;
use crate::settings_values::Control;

/// One change the settings window asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsChange {
    /// Shows a section; pressing Agents while it shows folds the sections inside it.
    Open(Section),
    /// Searches every setting for the words given.
    Find(String),
    /// Presses the control of a setting at an option: a switch flips, a choice is picked, a number goes down for 0 and up otherwise.
    Press(Setting, usize),
    /// Opens or closes the list of choices of a setting.
    TogglePicker(Setting),
    /// Narrows the open list of choices to the words given.
    Filter(String),
    /// Picks the choice at a place among all the options of a setting, and closes its list.
    Pick(Setting, usize),
    /// Has a setting written as text hold the text given.
    Text(Setting, String),
    /// Adds a line to a setting written as a list.
    AddLine(Setting, String),
    /// Takes the line at a place out of a setting written as a list.
    RemoveLine(Setting, usize),
    /// Puts a setting back as it comes.
    Reset(Setting),
    /// Searches the keymap's actions for the words given.
    KeymapFind(String),
    /// Searches the lists of customizations for the words given.
    ItemFind(String),
    /// Offers the agent of this id for new sessions, or leaves it out.
    ToggleAgent(String),
    /// Turns the tool server at a place on or off.
    ToggleMcp(usize),
    /// Takes the tool server at a place out.
    RemoveMcp(usize),
    /// Takes the model server of this id off the server's machine.
    RemoveModelServer(String),
    /// Installs the language or extension of this id, removes it when installed, or updates it when an update is out.
    InstallOrRemove(String),
    /// Turns the customization, tool set or tool kept under this key on or off.
    ToggleWithheld(String),
    /// Takes the known server at a place out of the servers page.
    RemoveServer(usize),
    /// Adds a server by its name, its address, the place of how it is reached among [`crate::settings_model::REACHES`], and the token it lets the window in with, kept for servers reached straight or through a tunnel.
    AddServer(String, String, usize, String),
    /// Takes every key off the action kept under this name.
    ClearKeys(String),
    /// Puts the action kept under this name back on the keys of the preset.
    ResetKeys(String),
    /// Binds the action kept under the first name to the binding written second, such as `ctrl+shift+k`, in place of its keys.
    SetKeys(String, String),
    /// Types or presses something in a form of the sections inside Agents.
    Form(crate::settings_forms::FormChange),
}

/// Whether `text` reads as an origin agents may be allowed to open: a host, optionally with a port, or a web address that begins with `http://` or `https://`, with no white space and no user name.
pub fn origin_reads(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    !rest.is_empty()
        && !rest.contains("://")
        && !text.contains(char::is_whitespace)
        && !rest.contains('@')
        && !rest.starts_with(['/', ':'])
}

/// Whether `text` reads as a vim binding: keys, `=>` and what they do.
pub fn binding_reads(text: &str) -> bool {
    text.split_once("=>")
        .is_some_and(|(keys, action)| !keys.trim().is_empty() && !action.trim().is_empty())
}

/// Whether `text` can be added as a line of the list `setting` holds: a vim binding, an origin, or any text that is not empty.
pub fn line_reads(setting: Setting, text: &str) -> bool {
    let text = text.trim();
    match setting {
        Setting::VimBindings => binding_reads(text),
        Setting::AgentOrigins => origin_reads(text),
        _ => !text.is_empty(),
    }
}

/// The request asking for the newest release on `channel`.
pub fn release_request(channel: pm_config::Channel) -> Request {
    Request::LatestRelease {
        channel: match channel {
            pm_config::Channel::Stable => "stable",
            pm_config::Channel::Preview => "preview",
        }
        .to_owned(),
    }
}

impl SettingsModel {
    /// The requests that fill in what `section` lists from the server.
    pub fn asks_for(&self, section: Section) -> Vec<Request> {
        match section {
            Section::McpServers | Section::Agents => vec![Request::McpServers],
            Section::ModelServers => vec![Request::ModelServers],
            Section::ModelSlots => vec![Request::ModelSlots {
                session: None,
                change: None,
            }],
            Section::Languages => vec![Request::Extensions],
            Section::Customizations
            | Section::Skills
            | Section::Instructions
            | Section::AgentDefinitions
            | Section::Hooks
            | Section::Tools => match &self.library_place {
                Some((agent, directory)) => vec![
                    Request::Library {
                        agent: agent.clone(),
                        directory: directory.clone(),
                    },
                    Request::ToolSets {
                        agent: agent.clone(),
                    },
                ],
                None => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// Takes `change` in and returns the requests the server must be sent for it.
    pub fn apply(&mut self, change: SettingsChange) -> Vec<Request> {
        let machine = self.machine.clone();
        let mut requests = self.take_in(change);
        if self.machine_loaded && self.machine != machine {
            requests.push(Request::SaveMachineSettings {
                settings: Box::new(self.machine.clone()),
            });
        }
        requests
    }

    /// Changes the model as `change` says and returns the requests it needs besides saving the machine's settings.
    fn take_in(&mut self, change: SettingsChange) -> Vec<Request> {
        match change {
            SettingsChange::Open(section) => {
                if section == Section::Agents {
                    let showing = self.page.section == section;
                    self.page.folded = showing && !self.page.folded;
                }
                self.page.section = section;
                self.page.find.clear();
                self.page.picker = None;
                self.page.item_find.clear();
                return self.asks_for(section);
            }
            SettingsChange::Find(text) => self.page.find = text,
            SettingsChange::Press(setting, option) => {
                if setting.is_text() || crate::settings_catalog::LISTS.contains(&setting) {
                    return Vec::new();
                }
                setting.press(self, option);
                if self.page.picker == Some(setting) {
                    self.page.picker = None;
                }
                return self.changed(setting);
            }
            SettingsChange::TogglePicker(setting) => {
                let open = self.page.picker == Some(setting);
                self.page.picker = (!open).then_some(setting);
                self.page.filter.clear();
            }
            SettingsChange::Filter(text) => self.page.filter = text,
            SettingsChange::Pick(setting, index) => {
                self.page.picker = None;
                self.page.filter.clear();
                match setting.control(self) {
                    Control::Choice(..) => setting.press(self, index),
                    _ => setting.pick(self, index),
                }
                return self.changed(setting);
            }
            SettingsChange::Text(setting, text) => {
                if setting.is_text() && text != setting.text(self) {
                    setting.set_text(self, text);
                    return self.changed(setting);
                }
            }
            SettingsChange::AddLine(setting, text) => {
                let text = text.trim().to_owned();
                let reads = line_reads(setting, &text);
                let mut lines = setting.lines(self);
                if reads && !lines.contains(&text) {
                    lines.push(text);
                    setting.set_lines(self, &lines);
                    return self.changed(setting);
                }
            }
            SettingsChange::RemoveLine(setting, place) => {
                let mut lines = setting.lines(self);
                if place < lines.len() {
                    lines.remove(place);
                    setting.set_lines(self, &lines);
                    return self.changed(setting);
                }
            }
            SettingsChange::Reset(setting) => {
                setting.reset(self);
                self.page.picker = None;
                return self.changed(setting);
            }
            SettingsChange::KeymapFind(text) => self.page.keymap_find = text,
            SettingsChange::ItemFind(text) => self.page.item_find = text,
            SettingsChange::ToggleAgent(id) => {
                let shown = self.shown_agents();
                let hidden = &mut self.machine.agents.hidden;
                if let Some(place) = hidden.iter().position(|held| *held == id) {
                    hidden.remove(place);
                } else if shown > 1 {
                    hidden.push(id);
                }
            }
            SettingsChange::ToggleMcp(index) => {
                if let Some(server) = self.mcp_servers.get_mut(index) {
                    server.enabled = !server.enabled;
                    return vec![self.save_mcp()];
                }
            }
            SettingsChange::RemoveMcp(index) => {
                if index < self.mcp_servers.len() {
                    self.mcp_servers.remove(index);
                    return vec![self.save_mcp()];
                }
            }
            SettingsChange::RemoveModelServer(id) => {
                return vec![Request::RemoveModelServer { id }];
            }
            SettingsChange::InstallOrRemove(id) => {
                return self.install_or_remove(&id).into_iter().collect();
            }
            SettingsChange::ToggleWithheld(key) => {
                match self.withheld.iter().position(|held| *held == key) {
                    Some(place) => {
                        self.withheld.remove(place);
                    }
                    None => self.withheld.push(key),
                }
                return vec![Request::SaveWithheld {
                    keys: self.withheld.clone(),
                }];
            }
            SettingsChange::RemoveServer(index) => {
                let servers = &mut self.window.servers;
                if index < servers.known.len() {
                    let removed = servers.known.remove(index);
                    if servers.default_server.as_deref() == Some(removed.name.as_str()) {
                        servers.default_server = None;
                    }
                }
            }
            SettingsChange::AddServer(name, address, reach, token) => {
                let name = name.trim().to_owned();
                let servers = &mut self.window.servers;
                let taken =
                    name == "This machine" || servers.known.iter().any(|held| held.name == name);
                if !name.is_empty() && !taken {
                    let reach = crate::settings_model::REACHES
                        .get(reach)
                        .map_or_else(Default::default, |(reach, _)| *reach);
                    let token = if reach.keeps_token() {
                        token.trim().to_owned()
                    } else {
                        String::new()
                    };
                    servers.known.push(pm_config::KnownServer {
                        name,
                        reach,
                        address: address.trim().to_owned(),
                        token,
                    });
                }
            }
            SettingsChange::ClearKeys(key) => {
                self.editor.keys.insert(key, Vec::new());
            }
            SettingsChange::SetKeys(key, binding) => {
                self.editor.keys.insert(key, vec![binding]);
            }
            SettingsChange::ResetKeys(key) => {
                self.editor.keys.remove(&key);
            }
            SettingsChange::Form(change) => return self.take_form(change),
        }
        Vec::new()
    }

    /// The request that saves the tool servers as they stand.
    fn save_mcp(&self) -> Request {
        Request::SaveMcpServers {
            servers: self.mcp_servers.clone(),
        }
    }

    /// The request that installs, updates or removes the extension `id`, unless it is busy.
    fn install_or_remove(&self, id: &str) -> Option<Request> {
        let extension = self.extensions.iter().find(|held| held.id == id)?;
        if extension.busy.is_some() {
            return None;
        }
        Some(if extension.installed && extension.update.is_none() {
            Request::RemoveExtension {
                id: extension.id.clone(),
            }
        } else {
            Request::InstallExtensions {
                ids: vec![extension.id.clone()],
            }
        })
    }

    /// The requests `setting` needs once it changed: looking for a newer release again when how that is done changed.
    fn changed(&mut self, setting: Setting) -> Vec<Request> {
        match setting {
            Setting::Channel | Setting::CheckUpdates => {
                self.release = None;
                if self.window.general.check_updates {
                    vec![release_request(self.window.general.channel)]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// Takes in what the server answered about the newest release.
    pub fn take_release(&mut self, version: Option<String>, url: String) {
        self.release = Some(Release::judged(version, url));
    }
}

#[cfg(test)]
/// Tests for how the settings window's changes are taken in.
mod tests {
    use super::*;
    use crate::settings_catalog::LISTS;

    /// A model with the machine's settings loaded.
    fn loaded() -> SettingsModel {
        SettingsModel {
            machine_loaded: true,
            ..SettingsModel::new(Default::default(), Default::default())
        }
    }

    /// A switch the window keeps flips without a request; one the machine keeps is sent to the server.
    #[test]
    fn machine_settings_are_sent_and_window_ones_are_not() {
        let mut model = loaded();
        let before = model.window.general.confirm_close;
        assert!(
            model
                .apply(SettingsChange::Press(Setting::ConfirmClose, 0))
                .is_empty()
        );
        assert_ne!(model.window.general.confirm_close, before);
        let sent = model.apply(SettingsChange::Press(Setting::Memory, 0));
        assert!(matches!(
            sent.as_slice(),
            [Request::SaveMachineSettings { .. }]
        ));
    }

    /// Nothing the machine keeps is sent before the server told what it keeps.
    #[test]
    fn nothing_is_sent_before_the_machine_loaded() {
        let mut model = SettingsModel::default();
        assert!(
            model
                .apply(SettingsChange::Press(Setting::Memory, 0))
                .is_empty()
        );
    }

    /// A line is added to its own list, once, and only when it reads.
    #[test]
    fn lines_are_added_once_when_they_read() {
        let mut model = loaded();
        model.apply(SettingsChange::AddLine(
            Setting::AgentOrigins,
            "https://example.com".to_owned(),
        ));
        model.apply(SettingsChange::AddLine(
            Setting::AgentOrigins,
            "https://example.com".to_owned(),
        ));
        model.apply(SettingsChange::AddLine(
            Setting::AgentOrigins,
            "not an origin".to_owned(),
        ));
        assert_eq!(
            model.window.general.agent_origins,
            vec!["https://example.com".to_owned()]
        );
        model.apply(SettingsChange::AddLine(
            Setting::VimBindings,
            "g h => Hover".to_owned(),
        ));
        assert!(
            model
                .editor
                .vim_bindings
                .contains(&"g h => Hover".to_owned())
        );
        model.apply(SettingsChange::RemoveLine(Setting::AgentOrigins, 0));
        assert!(model.window.general.agent_origins.is_empty());
    }

    /// Resetting a setting puts it back as it comes.
    #[test]
    fn reset_puts_a_setting_back() {
        let mut model = loaded();
        model.apply(SettingsChange::Press(Setting::CodeSize, 1));
        assert!(model_changed(&model, Setting::CodeSize));
        model.apply(SettingsChange::Reset(Setting::CodeSize));
        assert!(!model_changed(&model, Setting::CodeSize));
    }

    /// Whether `setting` stands other than as it comes in `model`.
    fn model_changed(model: &SettingsModel, setting: Setting) -> bool {
        setting.changed(model)
    }

    /// Pressing Agents while it shows folds the sections inside it, and opening a section clears the search.
    #[test]
    fn agents_folds_and_sections_clear_the_search() {
        let mut model = loaded();
        model.apply(SettingsChange::Find("font".to_owned()));
        assert_eq!(model.showing(), Section::Results);
        model.apply(SettingsChange::Open(Section::Agents));
        assert_eq!(model.showing(), Section::Agents);
        assert!(!model.page.folded);
        model.apply(SettingsChange::Open(Section::Agents));
        assert!(model.page.folded);
    }

    /// Every row of choices is short enough to show beside its title, and every setting drawn as a list is one of the lists.
    #[test]
    fn choices_and_lists_fit() {
        let model = SettingsModel::default();
        for setting in Setting::ALL {
            match setting.control(&model) {
                Control::Choice(names, _) => assert!(names.len() <= 6, "{setting:?}"),
                Control::List(_) => assert!(LISTS.contains(&setting), "{setting:?}"),
                _ => {}
            }
        }
    }

    /// The last shown agent cannot be left out.
    #[test]
    fn the_last_agent_stays() {
        let mut model = loaded();
        let ids: Vec<String> = model.agents.iter().map(|agent| agent.id.clone()).collect();
        for id in &ids {
            model.apply(SettingsChange::ToggleAgent(id.clone()));
        }
        assert_eq!(model.shown_agents(), 1.min(ids.len()));
    }
}

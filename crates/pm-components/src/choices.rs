//! What the reader picks around the prompt box: the agent's settings, a command behind a slash, and where new work is done.

use pm_acp::{Discovery, Setting, Slash};
use pm_input::FocusId;
use pm_protocol::ExtensionCommand;

use crate::captures::{CAPTURE_BASE, CAPTURE_REMOVE_BASE, MOST_CAPTURES};
use crate::elements::{ELEMENT_BASE, ELEMENT_REMOVE_BASE, MOST_ELEMENTS};
use crate::state::{
    ACCOUNT, ACCOUNT_BASE, AGENT_TOGGLE, APPROVALS_CHIP, ATTACHED_BASE, Activity, BRANCH_BASE,
    DETAILS_BASE, EFFORT_BASE, LOCATION, Location, MENTION, MENU_BODY, Outgoing, PICK_BASE,
    PLACE_BASE, PLAN_CHIP, Page, Picker, SETTING_BASE, SHELLS_CHIP, SKILL_BUTTON, SLASH_BASE,
    SLASH_BUTTON, STEP_BASE, SUBAGENTS_CHIP, WorkspaceState, prompt_field,
};

/// How tall one value of a menu is, in logical pixels.
pub const ROW_HEIGHT: f32 = 47.0;

/// The height in logical pixels of one row of the list of files.
const MENTION_ROW_HEIGHT: f32 = 34.0;

/// The categories of settings shown as buttons in the prompt box, in the order they are shown.
const SHOWN: [&str; 2] = ["model", "mode"];

/// A server listing retained by machine and directory while another page is open.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileListing {
    /// The request key, including the remote machine identity.
    key: String,
    /// Files in server order, which tree action indices refer to.
    files: Vec<String>,
    /// Explicit folders, including empty ones.
    folders: Vec<String>,
    /// Paths styled as ignored.
    ignored: std::collections::HashSet<String>,
}

impl WorkspaceState {
    /// Whether pressing `action` leaves the open menu open or works it.
    pub fn keeps_picker(action: FocusId) -> bool {
        let FocusId(id) = action;
        action == LOCATION
            || action == MENU_BODY
            || action == ACCOUNT
            || (ACCOUNT_BASE..ACCOUNT_BASE + 50).contains(&id)
            || (SETTING_BASE..SETTING_BASE + 10).contains(&id)
            || (ELEMENT_BASE..ELEMENT_BASE + MOST_ELEMENTS as u64).contains(&id)
            || (CAPTURE_BASE..CAPTURE_BASE + MOST_CAPTURES as u64).contains(&id)
            || (PICK_BASE..PICK_BASE + 100).contains(&id)
            || (EFFORT_BASE..EFFORT_BASE + 50).contains(&id)
            || (PLACE_BASE..PLACE_BASE + 10).contains(&id)
            || (BRANCH_BASE..BRANCH_BASE + 100).contains(&id)
    }

    /// Whether pressing `action` leaves the open panel above the conversation's chips open.
    pub fn keeps_activity(action: FocusId) -> bool {
        action == PLAN_CHIP
            || action == APPROVALS_CHIP
            || action == SUBAGENTS_CHIP
            || action == SHELLS_CHIP
            || (STEP_BASE..DETAILS_BASE).contains(&action.0)
    }

    /// What the agent about to be spoken to lets the reader change: the open session's, or the chosen agent's.
    pub fn settings_now(&self) -> &[Setting] {
        match self.open() {
            Some(session) => &session.settings,
            None => self
                .agent_settings
                .get(self.agent)
                .map_or(&[], Vec::as_slice),
        }
    }

    /// The settings shown as buttons in the prompt box among `settings`, by position: the model, then the mode.
    pub fn chips(settings: &[Setting]) -> Vec<usize> {
        SHOWN
            .iter()
            .filter_map(|category| {
                settings
                    .iter()
                    .position(|setting| setting.category == *category)
            })
            .collect()
    }

    /// The position of the agent the prompt is for: the open session's, or the one chosen for new work.
    pub(crate) fn agent_now(&self) -> usize {
        self.open().map_or(self.agent, |session| session.agent)
    }

    /// Whether the agent the prompt is for offers skills behind a dollar sign.
    pub fn has_skills(&self) -> bool {
        self.agent_has_skills(self.agent_now())
    }

    /// Who the agent the prompt is for is signed in as: the name of the open session's login or the one new work starts with, or the one in its own default folder.
    pub fn account(&self) -> Option<&str> {
        self.account_as(self.agent_now(), self.login_in_use())
    }

    /// Who the agent at `agent` is signed in as under the login `login`: its name, or, for the agent's own, the one in its default folder.
    pub fn account_as(&self, agent: usize, login: Option<&str>) -> Option<&str> {
        let Some(login) = login else {
            return self.defaults.get(agent)?.as_ref()?.as_deref();
        };
        let owner = self.agent_id_at(agent)?;
        self.logins
            .iter()
            .find(|(by, id, _)| *by == owner && id == login)
            .map(|(_, _, name)| name.as_str())
    }

    /// Who the agent the prompt is for is signed in as in its own default folder, when its files say.
    pub fn default_account(&self) -> Option<&str> {
        self.account_as(self.agent_now(), None)
    }

    /// Whether the command named `name` is a skill the agent has installed.
    pub fn is_skill(&self, name: &str) -> bool {
        self.skill_names
            .contains(name.rsplit(':').next().unwrap_or(name))
    }

    /// The commands or skills offered behind a slash or a dollar sign that fit what has been typed, with which it was.
    pub fn slash_matches(&self) -> Vec<(char, Slash)> {
        let typed = self.prompt.text();
        let Some(prefix) = typed.chars().next() else {
            return Vec::new();
        };
        let rest = &typed[prefix.len_utf8()..];
        if rest.contains(char::is_whitespace) {
            return Vec::new();
        }
        let source: &[Slash] = match (prefix, self.open()) {
            ('/', Some(session)) => &session.commands,
            ('/', None) => self
                .agent_commands
                .get(self.agent)
                .map_or(&[], Vec::as_slice),
            ('$', Some(session)) if self.has_skills() => &session.skills,
            ('$', None) if self.has_skills() => &self.skills,
            _ => return Vec::new(),
        };
        let rest = rest.to_lowercase();
        let brought: Vec<Slash> = match prefix {
            '/' => self
                .extension_commands()
                .into_iter()
                .map(|(_, command)| Slash {
                    name: format!("{}:{}", command.extension, command.id),
                    description: command.description,
                })
                .collect(),
            _ => Vec::new(),
        };
        source
            .iter()
            .cloned()
            .chain(brought)
            .filter(|command| command.name.to_lowercase().starts_with(&rest))
            .map(|command| (prefix, command))
            .collect()
    }

    /// The command an extension brings that the name `name` typed behind a slash stands for, with the machine it runs on.
    fn extension_command_named(&self, name: &str) -> Option<(Option<String>, ExtensionCommand)> {
        self.extension_commands()
            .into_iter()
            .find(|(_, command)| format!("{}:{}", command.extension, command.id) == name)
    }

    /// The machine and folder the prompt is for: the open session's, or the project being started in.
    pub(crate) fn prompt_place(&self) -> Option<(Option<String>, String)> {
        match self.open() {
            Some(session) => Some((
                self.projects.get(session.project)?.host(),
                session.directory.clone(),
            )),
            None => self.projects.get(self.current_project()).map(|project| {
                (
                    project.host(),
                    project.directory().to_string_lossy().into_owned(),
                )
            }),
        }
    }

    /// Whether the folder the prompt is for is on the computer the window runs on, where the logins this window keeps apply.
    pub fn on_this_computer(&self) -> bool {
        self.prompt_place().is_none_or(|(host, _)| host.is_none())
    }

    /// Asks for the files of the folder the prompt is for when they are not the ones held or already asked for.
    pub fn refresh_files(&mut self) {
        let wanted = self.prompt_place();
        let key = wanted
            .as_ref()
            .map(|(host, directory)| format!("{host:?}:{directory}"));
        if self.files_of == key {
            return;
        }
        let cached = self
            .file_listings
            .iter()
            .find(|listing| Some(&listing.key) == key.as_ref());
        self.files = cached.map_or_else(Vec::new, |listing| listing.files.clone());
        self.tree_folders = cached.map_or_else(Vec::new, |listing| listing.folders.clone());
        self.tree_ignored = cached.map_or_else(Default::default, |listing| listing.ignored.clone());
        self.files_of = key.clone();
        if let (Some((host, directory)), Some(key)) = (wanted, key) {
            self.outbox.push(Outgoing::Files {
                key,
                host,
                directory,
                expanded: Vec::new(),
            });
        }
    }

    /// Whether the active directory is awaiting its first server listing.
    pub fn files_loading(&self) -> bool {
        self.files_of
            .as_ref()
            .is_some_and(|key| !self.file_listings.iter().any(|listing| &listing.key == key))
    }

    /// Retains a server listing for later navigation and displays it when its directory is active.
    pub fn with_files(
        mut self,
        key: &str,
        files: Vec<String>,
        folders: Vec<String>,
        ignored: Vec<String>,
    ) -> Self {
        let listing = std::sync::Arc::new(FileListing {
            key: key.to_owned(),
            files,
            folders,
            ignored: ignored.into_iter().collect(),
        });
        self.file_listings.retain(|held| held.key != key);
        self.file_listings.push_front(listing.clone());
        self.file_listings.truncate(16);
        if self.files_of.as_deref() == Some(key) {
            self.files = listing.files.clone();
            self.tree_folders = listing.folders.clone();
            self.tree_ignored = listing.ignored.clone();
        }
        self
    }

    /// The state holding `branches` for the project at `project`, unless the page has moved on.
    pub fn with_branches(mut self, project: usize, branches: Vec<String>) -> Self {
        if self.branches_of == Some(project) {
            self.branches = branches;
            if self.branches.is_empty() {
                self.location = Location::Local;
            }
            if self
                .base
                .as_ref()
                .is_some_and(|base| !self.branches.contains(base))
            {
                self.base = None;
            }
        }
        self
    }

    /// Stores discovered skills in matching conversations, including those opened before discovery finished.
    pub fn with_skills(
        mut self,
        agent: usize,
        project: usize,
        skills: Vec<pm_acp::Slash>,
        names: std::collections::HashSet<String>,
    ) -> Self {
        for session in &mut self.sessions {
            if session.agent == agent && session.project == project {
                session.skills = skills.clone();
            }
        }
        if self.skills_of == Some((agent, project)) {
            self.skills = skills;
            self.skill_names = names;
        }
        self
    }

    /// The state after the agent at `agent` on the machine `host` told what it lets the reader change, held now when those are the settings showing and kept for later when they are not.
    pub fn with_discovery_for(
        mut self,
        host: Option<String>,
        agent: usize,
        found: Discovery,
    ) -> Self {
        if host == self.discovery_host {
            return self.with_discovery(agent, found);
        }
        let slots = self.agent_slots();
        let kept = self
            .discovered_elsewhere
            .entry(host)
            .or_insert_with(|| (vec![Vec::new(); slots], vec![Vec::new(); slots]));
        if let Some(held) = kept.0.get_mut(agent) {
            *held = found.settings;
        }
        if let Some(held) = kept.1.get_mut(agent) {
            *held = found.commands;
        }
        self
    }

    /// Switches the settings and commands showing to those of the machine `host`, asking its agents when it has not been asked.
    fn sync_discovery(&mut self, host: Option<String>) {
        if self.discovery_host == host {
            return;
        }
        let now = (
            std::mem::take(&mut self.agent_settings),
            std::mem::take(&mut self.agent_commands),
        );
        self.discovered_elsewhere
            .insert(self.discovery_host.clone(), now);
        self.discovery_host = host.clone();
        let asked = self.discovered_elsewhere.remove(&host);
        let known = asked.is_some();
        let slots = self.agent_slots();
        let (settings, commands) =
            asked.unwrap_or_else(|| (vec![Vec::new(); slots], vec![Vec::new(); slots]));
        self.agent_settings = settings;
        self.agent_commands = commands;
        if !known {
            for agent in 0..slots {
                self.outbox.push(Outgoing::Discover {
                    agent,
                    host: host.clone(),
                });
            }
        }
    }

    /// The state knowing who the agent at `agent` is signed in as, and naming the login `naming` after the one asked about when there is one; `selected` is who the login in use is when nothing is named.
    pub fn with_identity(
        mut self,
        agent: usize,
        default: Option<String>,
        selected: Option<String>,
        naming: Option<String>,
    ) -> Self {
        if let Some(held) = self.defaults.get_mut(agent) {
            *held = Some(default);
        }
        match naming {
            Some(login) => self.name_login(agent, &login, selected),
            None => {
                if let Some(held) = self.identities.get_mut(agent) {
                    *held = Some(selected);
                }
            }
        }
        self
    }

    /// The word being typed at the end of the prompt, when it starts with an `@`, without the `@`.
    pub fn mention_query(&self) -> Option<String> {
        let typed = self.prompt.text();
        let word = typed.rsplit(char::is_whitespace).next()?;
        word.strip_prefix('@').map(str::to_lowercase)
    }

    /// The files that fit the word after the `@` being typed, those whose name starts with it first.
    pub fn mention_matches(&self) -> Vec<&String> {
        let Some(query) = self.mention_query() else {
            return Vec::new();
        };
        let name = |path: &String| path.rsplit('/').next().unwrap_or(path).to_lowercase();
        let mut found: Vec<&String> = self
            .files
            .iter()
            .filter(|path| path.to_lowercase().contains(&query))
            .collect();
        found.sort_by_key(|path| !name(path).starts_with(&query));
        found.truncate(500);
        found
    }

    /// Whether a list of commands or files is showing over the prompt.
    pub fn slash_open(&self) -> bool {
        !self.slash_matches().is_empty() || !self.mention_matches().is_empty()
    }

    /// Scrolls the list of files just far enough for the highlighted file to show.
    fn keep_mention_in_view(&mut self) {
        if !self.slash_matches().is_empty() {
            return;
        }
        let top = self.slash as f32 * MENTION_ROW_HEIGHT;
        let window = MENTION_ROW_HEIGHT * 8.0;
        if top < self.menu_scroll {
            self.menu_scroll = top;
        } else if top + MENTION_ROW_HEIGHT > self.menu_scroll + window {
            self.menu_scroll = top + MENTION_ROW_HEIGHT - window;
        }
    }

    /// The state with the highlight in the list of commands moved by `delta` rows, wrapping round.
    pub fn slash_moved(mut self, delta: isize) -> Self {
        let count = self.slash_matches().len().max(self.mention_matches().len()) as isize;
        if count > 0 {
            self.slash = (self.slash as isize + delta).rem_euclid(count) as usize;
            self.keep_mention_in_view();
        }
        self
    }

    /// The state with the highlighted command typed into the prompt.
    pub fn slash_taken(mut self) -> Self {
        let index = self.slash;
        self.take_slash(index);
        self
    }

    /// Types the command at `index` of those offered into the prompt, ready for its arguments.
    fn take_slash(&mut self, index: usize) {
        if self.slash_matches().is_empty() {
            let chosen = self
                .mention_matches()
                .get(index)
                .map(|path| (*path).clone());
            if let Some(path) = chosen {
                let typed = self.prompt.text().to_owned();
                let start = typed.rfind(char::is_whitespace).map_or(0, |at| {
                    at + typed[at..].chars().next().map_or(1, char::len_utf8)
                });
                self.prompt = prompt_field(&format!("{}@{path} ", &typed[..start]));
                self.slash = 0;
            }
            return;
        }
        let taken = self.slash_matches().get(index).cloned();
        if let Some((_, command)) = &taken
            && let Some((host, brought)) = self.extension_command_named(&command.name)
        {
            self.prompt = prompt_field("");
            self.slash = 0;
            self.run_extension_command(host, brought);
            return;
        }
        let chosen = taken.map(|(prefix, command)| format!("{prefix}{} ", command.name));
        if let Some(typed) = chosen {
            self.prompt = prompt_field(&typed);
            self.slash = 0;
        }
    }

    /// The state with the mode changed to the one after the current.
    pub fn mode_cycled(mut self) -> Self {
        let settings = self.settings_now();
        let Some(setting) = settings.iter().find(|setting| setting.category == "mode") else {
            return self;
        };
        if setting.picks.is_empty() {
            return self;
        }
        let at = setting
            .picks
            .iter()
            .position(|pick| pick.value == setting.current)
            .unwrap_or(0);
        let (id, value) = (
            setting.id.clone(),
            setting.picks[(at + 1) % setting.picks.len()].value.clone(),
        );
        self.change(&id, &value);
        self
    }

    /// Gives the setting `id` the value `value`, for now as the reader sees
    /// it, and then for the agent: at once when there is one, and at the
    /// start of the work when there is not yet.
    fn change(&mut self, id: &str, value: &str) {
        let open = self.open_index();
        let held = match open {
            Some(session) => self
                .sessions
                .get_mut(session)
                .map(|session| &mut session.settings),
            None => self.agent_settings.get_mut(self.agent),
        };
        let Some(setting) = held.and_then(|held| held.iter_mut().find(|setting| setting.id == id))
        else {
            return;
        };
        setting.current = value.to_owned();
        let agent = self.agent_now();
        self.remember(agent, id, value);
        match open {
            Some(session) => self.outbox.push(Outgoing::Set {
                session,
                setting: id.to_owned(),
                value: value.to_owned(),
            }),
            None => {
                let key = (self.agent, id.to_owned());
                if !self.touched.contains(&key) {
                    self.touched.push(key);
                }
            }
        }
    }

    /// Keeps `value` as what the reader chose for the setting `id` of the agent at `agent`.
    fn remember(&mut self, agent: usize, id: &str, value: &str) {
        let Some(agent) = self.agent_id_at(agent) else {
            return;
        };
        let held = self
            .choices
            .iter_mut()
            .find(|(held_agent, setting, _)| *held_agent == agent && setting == id);
        match held {
            Some(held) => held.2 = value.to_owned(),
            None => self.choices.push((agent, id.to_owned(), value.to_owned())),
        }
    }

    /// The settings the reader changed before starting work with the chosen agent, as the agent names them and their values.
    pub fn settings_changed(&self) -> Vec<(String, String)> {
        self.touched
            .iter()
            .filter(|(agent, _)| *agent == self.agent)
            .filter_map(|(_, id)| {
                self.agent_settings
                    .get(self.agent)?
                    .iter()
                    .find(|setting| setting.id == *id)
                    .map(|setting| (setting.id.clone(), setting.current.clone()))
            })
            .collect()
    }

    /// The state after the agent `id` on the machine `host` told what it lets the reader change and what commands it offers, when the list still has it.
    pub fn with_discovery_of(self, host: Option<String>, id: &str, found: Discovery) -> Self {
        match self.agent_position(id) {
            Some(agent) => self.with_discovery_for(host, agent, found),
            None => self,
        }
    }

    /// The state after the agent at `agent` told what it lets the reader change and what commands it offers.
    pub fn with_discovery(mut self, agent: usize, found: Discovery) -> Self {
        let id = self.agent_id_at(agent).unwrap_or_default();
        if let Some(held) = self.agent_settings.get_mut(agent) {
            *held = found.settings;
            for (_, setting, value) in self.choices.iter().filter(|(held, ..)| *held == id) {
                let Some(held) = held.iter_mut().find(|held| held.id == *setting) else {
                    continue;
                };
                if held.current != *value && held.picks.iter().any(|pick| pick.value == *value) {
                    held.current = value.clone();
                    let key = (agent, setting.clone());
                    if !self.touched.contains(&key) {
                        self.touched.push(key);
                    }
                }
            }
        }
        if let Some(held) = self.agent_commands.get_mut(agent) {
            *held = found.commands;
        }
        self
    }

    /// Reads skills for the prompt's agent and project, also hydrating restored conversations from the cache.
    fn sync_skills(&mut self) {
        let (agent, project) = match self.page {
            Page::New(project) => (self.agent, project),
            Page::Session(index) => {
                let Some(session) = self.sessions.get(index) else {
                    return;
                };
                (session.agent, session.project)
            }
            _ => return,
        };
        if self.skills_of == Some((agent, project)) {
            if let Page::Session(index) = self.page {
                self.sessions[index].skills = self.skills.clone();
            }
            return;
        }
        let Some(held) = self.projects.get(project) else {
            return;
        };
        self.skills_of = Some((agent, project));
        self.skills.clear();
        self.skill_names.clear();
        self.outbox.push(Outgoing::Skills {
            agent,
            project,
            host: held.host(),
            directory: held.directory().to_string_lossy().into_owned(),
        });
    }

    /// Asks who every agent is signed in as in its own folder, for those not asked yet, so that the list of accounts names each default account.
    pub(crate) fn ask_default_accounts(&mut self) {
        for agent in 0..self.agent_slots() {
            let Some(held) = self.defaults.get_mut(agent) else {
                continue;
            };
            if held.is_some() {
                continue;
            }
            *held = Some(None);
            if let Some(identity) = self.identities.get_mut(agent) {
                identity.get_or_insert(None);
            }
            self.outbox.push(Outgoing::Identity {
                agent,
                naming: None,
            });
        }
    }

    /// Reads what the page shown needs of its project: on a project's overview its own folder, and on the page for starting work also its branches, when they are not the ones read already.
    pub fn sync_context(&mut self) {
        self.sync_skills();
        if let Page::Project(project) = self.page {
            self.read_home(project);
            self.sync_files_owner();
            return;
        }
        let Page::New(project) = self.page else {
            self.sync_files_owner();
            self.homed = None;
            return;
        };
        self.read_home(project);
        self.sync_files_owner();
        if let Some(held) = self.projects.get(project) {
            self.sync_discovery(held.host());
        }
        let asked = self.defaults.get(self.agent).is_some_and(Option::is_some)
            && self.identities.get(self.agent).is_some_and(Option::is_some);
        if !asked && self.agent < self.agent_slots() {
            if let Some(held) = self.defaults.get_mut(self.agent) {
                held.get_or_insert(None);
            }
            if let Some(held) = self.identities.get_mut(self.agent) {
                held.get_or_insert(None);
            }
            self.outbox.push(Outgoing::Identity {
                agent: self.agent,
                naming: None,
            });
        }
        let Some(held) = self.projects.get(project) else {
            return;
        };
        if self.branches_of == Some(project) {
            return;
        }
        let ask = Outgoing::Branches {
            project,
            host: held.host(),
            directory: held.directory().to_string_lossy().into_owned(),
        };
        self.branches_of = Some(project);
        self.base = None;
        self.branches = Vec::new();
        self.location = Location::Local;
        self.outbox.push(ask);
    }

    /// Asks again which branches the project being started in has, so that the list is never older than the menu showing it.
    fn reread_branches(&mut self) {
        let Page::New(project) = self.page else {
            return;
        };
        if let Some(held) = self.projects.get(project) {
            self.outbox.push(Outgoing::Branches {
                project,
                host: held.host(),
                directory: held.directory().to_string_lossy().into_owned(),
            });
        }
    }

    /// Whether the project being started in is a repository, so that work can be done in a worktree.
    pub fn is_repository(&self) -> bool {
        !self.branches.is_empty()
    }

    /// Whether the project being started in offers a dev container.
    pub fn offers_container(&self) -> bool {
        match self.page {
            Page::New(project) => self
                .projects
                .get(project)
                .is_some_and(|held| held.devcontainer),
            _ => false,
        }
    }

    /// Whether new work is to run in a dev container: the reader asked for one and the project offers it.
    pub fn container_for_new_work(&self) -> bool {
        self.container && self.offers_container()
    }

    /// The branch the worktree of new work is made from, when the work is to be done in one.
    pub fn base_for_new_work(&self) -> Option<String> {
        (self.location == Location::Worktree && self.is_repository())
            .then(|| self.base.clone().or_else(|| self.branches.first().cloned()))
            .flatten()
    }

    /// What pressing the numbered button `id` among those around the prompt box does, if it is one of them.
    pub fn press_choice(&mut self, id: u64) -> bool {
        let within = |base: u64, count: usize| {
            id.checked_sub(base)
                .map(|offset| offset as usize)
                .filter(|offset| *offset < count)
        };
        if let Some(index) = within(ATTACHED_BASE, 50) {
            self.detach(index);
        } else if let Some(index) = within(ELEMENT_BASE, MOST_ELEMENTS) {
            let wanted = (index < self.elements.len()).then_some(Picker::Element(index));
            self.picker = if self.picker == wanted { None } else { wanted };
            self.menu_scroll = 0.0;
        } else if let Some(index) = within(ELEMENT_REMOVE_BASE, MOST_ELEMENTS) {
            self.remove_element(index);
        } else if let Some(index) = within(CAPTURE_BASE, MOST_CAPTURES) {
            let wanted = (index < self.captures.len()).then_some(Picker::Capture(index));
            self.picker = if self.picker == wanted { None } else { wanted };
            self.menu_scroll = 0.0;
        } else if let Some(index) = within(CAPTURE_REMOVE_BASE, MOST_CAPTURES) {
            self.remove_capture(index);
        } else if id == SLASH_BUTTON.0 {
            self.prompt = prompt_field("/");
            self.slash = 0;
        } else if id == MENTION.0 {
            let mut typed = self.prompt.text().to_owned();
            if !typed.is_empty() && !typed.ends_with(char::is_whitespace) {
                typed.push(' ');
            }
            typed.push('@');
            self.prompt = prompt_field(&typed);
            self.slash = 0;
        } else if id == SKILL_BUTTON.0 {
            self.prompt = prompt_field("$");
            self.slash = 0;
        } else if id == LOCATION.0 {
            self.picker = (self.picker != Some(Picker::Location)).then_some(Picker::Location);
            if self.picker.is_some() {
                self.reread_branches();
            }
        } else if id == ACCOUNT.0 {
            self.picker = (self.picker != Some(Picker::Account)).then_some(Picker::Account);
        } else if let Some(index) = within(ACCOUNT_BASE, 50) {
            self.pick_account(index);
        } else if id == PLAN_CHIP.0 {
            self.activity = (self.activity != Some(Activity::Plan)).then_some(Activity::Plan);
        } else if id == APPROVALS_CHIP.0 {
            self.activity =
                (self.activity != Some(Activity::Approvals)).then_some(Activity::Approvals);
        } else if id == SUBAGENTS_CHIP.0 {
            self.activity =
                (self.activity != Some(Activity::Subagents)).then_some(Activity::Subagents);
        } else if id == SHELLS_CHIP.0 {
            self.activity = (self.activity != Some(Activity::Shells)).then_some(Activity::Shells);
        } else if let Some(index) = within(SETTING_BASE, SHOWN.len()) {
            let shown = Self::chips(self.settings_now());
            let wanted = shown.get(index).copied().map(Picker::Setting);
            self.picker = if self.picker == wanted { None } else { wanted };
            self.menu_scroll = self.scroll_to_current();
        } else if let Some(index) = within(PICK_BASE, 100) {
            self.pick(index);
        } else if id == AGENT_TOGGLE.0 {
            self.toggle_agent();
        } else if let Some(index) = within(EFFORT_BASE, 50) {
            self.pick_effort(index);
        } else if id == PLACE_BASE + 2 {
            self.container = !self.container;
        } else if let Some(index) = within(PLACE_BASE, 2) {
            self.location = if index == 0 {
                Location::Worktree
            } else {
                Location::Local
            };
            if self.location == Location::Local && !self.offers_container() {
                self.picker = None;
            }
        } else if let Some(index) = within(BRANCH_BASE, self.branches.len()) {
            self.base = Some(self.branches[index].clone());
            self.location = Location::Worktree;
            self.picker = None;
        } else if let Some(index) = within(SLASH_BASE, 50) {
            self.take_slash(index);
        } else {
            return false;
        }
        true
    }

    /// The agent's id the account menu is for: the open session's, or the one chosen for new work.
    fn agent_id(&self) -> Option<String> {
        self.agent_id_at(self.agent_now())
    }

    /// The named logins of the agent the account menu is for: the folder name and what each is called.
    pub fn logins_here(&self) -> Vec<(&str, &str)> {
        let Some(agent) = self.agent_id() else {
            return Vec::new();
        };
        self.logins
            .iter()
            .filter(|(owner, _, _)| *owner == agent)
            .map(|(_, id, name)| (id.as_str(), name.as_str()))
            .collect()
    }

    /// The folder name of the login the open session runs as, or, with none open, the one new work of the chosen agent starts with; nothing for the agent's own.
    pub fn login_in_use(&self) -> Option<&str> {
        if let Some(session) = self.open() {
            return session.login.as_deref();
        }
        self.login_for_new_work(self.agent)
    }

    /// The folder name of the login new sessions of the agent at `agent` start as, if it is not the agent's own.
    pub(crate) fn login_for_new_work(&self, agent: usize) -> Option<&str> {
        let agent = self.agent_id_at(agent)?;
        self.in_use
            .iter()
            .find(|(owner, _)| *owner == agent)
            .map(|(_, id)| id.as_str())
    }

    /// Takes the row `index` of the account menu: the agent's own login, a named one, or adding one; for the open session only, or with none open, for new work.
    fn pick_account(&mut self, index: usize) {
        let Some(agent) = self.agent_id() else {
            return;
        };
        let logins: Vec<String> = self
            .logins_here()
            .into_iter()
            .map(|(id, _)| id.to_owned())
            .collect();
        self.picker = None;
        let login = match index {
            0 => None,
            _ => match logins.get(index - 1) {
                Some(id) => Some(id.clone()),
                None => {
                    self.add_account(self.agent_now());
                    let added = self.in_use.iter().find(|(owner, _)| *owner == agent);
                    let added = added.map(|(_, id)| id.clone());
                    self.set_session_login(added);
                    return;
                }
            },
        };
        if self.open().is_some() {
            self.set_session_login(login);
            return;
        }
        self.in_use.retain(|(owner, _)| *owner != agent);
        if let Some(id) = login {
            self.in_use.push((agent.to_owned(), id));
        }
        self.identities[self.agent] = None;
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
    }

    /// Has the open session run as the login `login`, or as the agent's own when there is none; nothing when no session is open.
    fn set_session_login(&mut self, login: Option<String>) {
        let Some(session) = self.open_index() else {
            return;
        };
        if let Some(held) = self.sessions.get_mut(session) {
            held.login = login.clone();
            self.outbox
                .push(Outgoing::SessionAccount { session, login });
        }
    }

    /// Adds a login for the agent at `agent`, makes it the one in use, and has the server sign it in.
    pub(crate) fn add_account(&mut self, agent: usize) {
        let Some(owner) = self.agent_id_at(agent) else {
            return;
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        let id = format!("{stamp:x}");
        let name = self.placeholder_name(&owner);
        self.in_use.retain(|(by, _)| *by != owner);
        self.logins.push((owner.clone(), id.clone(), name));
        self.in_use.push((owner, id.clone()));
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
        self.outbox.push(Outgoing::SignIn {
            agent,
            login: Some(id),
        });
        if let Some(identity) = self.identities.get_mut(agent) {
            *identity = None;
        }
    }

    /// The name a new login of the agent `owner` has until its sign-in tells who it is: the lowest number free of the logins' own, counting the agent's default account as the first.
    fn placeholder_name(&self, owner: &str) -> String {
        let number = (2..)
            .find(|number| {
                let name = format!("Account {number}");
                !self
                    .logins
                    .iter()
                    .any(|(by, _, held)| by == owner && *held == name)
            })
            .unwrap_or(2);
        format!("Account {number}")
    }

    /// Takes the `index`-th login of the list out, and has the server keep the list and delete its sign-in.
    pub(crate) fn remove_account(&mut self, index: usize) {
        if index >= self.logins.len() {
            return;
        }
        let (owner, id, _) = self.logins.remove(index);
        self.in_use
            .retain(|(by, used)| !(*by == owner && *used == id));
        for session in &mut self.sessions {
            if session.login.as_ref() == Some(&id) {
                session.login = None;
            }
        }
        if let Some(agent) = self.agent_position(&owner)
            && let Some(identity) = self.identities.get_mut(agent)
        {
            *identity = None;
        }
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
    }

    /// Adds the sessions the server of `host` holds that belong to the projects here, asks it for the submodules of those projects, and returns each one's position and the server's number for it.
    pub fn restore_sessions(
        &mut self,
        host: &Option<String>,
        infos: &[pm_protocol::SessionInfo],
    ) -> Vec<(usize, u64)> {
        let mut added = Vec::new();
        for project in 0..self.projects.len() {
            if self.projects[project].host() == *host {
                self.ask_submodules(project);
            }
        }
        for info in infos {
            let Some(project) = self.projects.iter().position(|held| {
                held.host() == *host && held.directory().to_string_lossy() == info.repository
            }) else {
                continue;
            };
            let agent = self.agent_position_or_retire(&info.agent);
            self.sessions.push(crate::sample::Session::restored(
                &info.title,
                project,
                &crate::sample::Session::checkout_label(
                    info.branch.as_deref(),
                    info.head.as_deref(),
                    info.base.as_deref(),
                ),
                &info.directory,
                info.base.clone(),
                agent,
                info.closed,
            ));
            if let Some(session) = self.sessions.last_mut() {
                session.updated_at = info.updated;
                session.login = info.login.clone();
                session.submodule = info.submodule.clone();
                session.directory = session.folder_in(&info.directory);
            }
            added.push((self.sessions.len() - 1, info.session));
        }
        added
    }

    /// The state with the files at `paths` attached to the prompt, each once.
    pub fn with_attachments(mut self, paths: &[String]) -> Self {
        for path in paths {
            if !self.attachments.contains(path) {
                self.attachments.push(path.clone());
            }
        }
        self
    }

    /// The state knowing what the picture at `path` looks like.
    pub fn with_preview(mut self, path: &str, picture: pm_ui::Picture) -> Self {
        self.previews.insert(path.to_owned(), picture);
        self
    }

    /// The state with an element of a web page picked for the prompt, unless it is the same as one already there or there are too many.
    pub fn with_element(mut self, element: crate::elements::Element) -> Self {
        if self.elements.len() < MOST_ELEMENTS && !self.elements.contains(&element) {
            self.elements.push(element);
        }
        self
    }

    /// The state without the picked element at `index`.
    fn remove_element(&mut self, index: usize) {
        if index < self.elements.len() {
            self.elements.remove(index);
        }
        if matches!(self.picker, Some(Picker::Element(_))) {
            self.picker = None;
        }
    }

    /// The state without the attachment at `index`.
    fn detach(&mut self, index: usize) {
        if index < self.attachments.len() {
            self.attachments.remove(index);
        }
    }

    /// The state with the logins the reader added before and the one in use for each agent.
    pub fn with_accounts(
        mut self,
        logins: Vec<(String, String, String)>,
        in_use: Vec<(String, String)>,
    ) -> Self {
        self.logins = logins;
        self.in_use = in_use;
        self
    }

    /// Names the login `used` of the agent after who it is now signed in as, when it still has the name it was given on adding; it also tells who the agent is signed in as when that login is the one in use.
    pub fn name_login(&mut self, agent: usize, used: &str, who: Option<String>) {
        let Some(held) = self.agent_id_at(agent) else {
            return;
        };
        if self.login_for_new_work(agent) == Some(used)
            && let Some(identity) = self.identities.get_mut(agent)
        {
            *identity = Some(who.clone());
        }
        let Some(who) = who else {
            return;
        };
        if let Some(login) = self
            .logins
            .iter_mut()
            .find(|(owner, id, name)| *owner == held && id == used && name.starts_with("Account "))
        {
            login.2 = who;
            self.outbox.push(Outgoing::Accounts {
                logins: self.logins.clone(),
                in_use: self.in_use.clone(),
            });
        }
    }

    /// Asks who the agent is signed in as under the login `login`, to name that login after it.
    pub fn outbox_identity(&mut self, agent: usize, login: Option<String>) {
        self.outbox.push(Outgoing::Identity {
            agent,
            naming: login,
        });
    }

    /// Forgets who the agent is signed in as, so that it is looked up again.
    pub fn forget_identity(&mut self, agent: usize) {
        if let Some(held) = self.identities.get_mut(agent) {
            *held = None;
        }
    }

    /// How far to scroll the open menu so that the value it holds now shows.
    fn scroll_to_current(&self) -> f32 {
        let Some(Picker::Setting(at)) = self.picker else {
            return 0.0;
        };
        let Some(setting) = self.settings_now().get(at) else {
            return 0.0;
        };
        let place = setting
            .picks
            .iter()
            .position(|pick| pick.value == setting.current)
            .unwrap_or(0);
        (place as f32 * ROW_HEIGHT - ROW_HEIGHT).max(0.0)
    }

    /// Gives the setting of the agent at `index` among those it offers now the value `value`.
    pub fn agent_setting_changed(&mut self, index: usize, value: &str) {
        let Some(id) = self
            .settings_now()
            .get(index)
            .map(|setting| setting.id.clone())
        else {
            return;
        };
        self.change(&id, value);
    }

    /// Chooses the value at `index` of the menu that is open.
    fn pick(&mut self, index: usize) {
        let Some(Picker::Setting(at)) = self.picker else {
            return;
        };
        let Some(setting) = self.settings_now().get(at) else {
            return;
        };
        if let Some(pick) = setting.picks.get(index) {
            let (id, value) = (setting.id.clone(), pick.value.clone());
            self.change(&id, &value);
        }
        self.picker = None;
    }

    /// Switches the agent persona setting to the value it does not have now.
    fn toggle_agent(&mut self) {
        let Some(setting) = self
            .settings_now()
            .iter()
            .find(|setting| setting.id == "agent")
        else {
            return;
        };
        let other = setting
            .picks
            .iter()
            .find(|pick| pick.value != setting.current)
            .map(|pick| (setting.id.clone(), pick.value.clone()));
        if let Some((id, value)) = other {
            self.change(&id, &value);
        }
    }

    /// The box to give keyboard focus after `id` was pressed, when that is the prompt or the field for an answer of the reader's own.
    pub fn refocus(id: FocusId) -> Option<FocusId> {
        let FocusId(number) = id;
        let slash_row = (SLASH_BASE..SLASH_BASE + 50).contains(&number);
        if id == crate::draft::TYPE_OWN {
            return Some(crate::draft::OTHER);
        }
        if crate::adder::is_adder(id) {
            return Some(crate::adder::ADDER_FIELD);
        }
        if crate::folder_picker::is_picker(id) {
            return Some(crate::folder_picker::PICKER_FIELD);
        }
        if crate::project_search::is_search_part(id)
            && id != crate::project_search::PROJECT_SEARCH_FIELD
            && number < crate::project_search::SEARCH_GROUP_BASE
        {
            return Some(crate::project_search::PROJECT_SEARCH_FIELD);
        }
        if id == crate::git_controls::BRANCH_BUTTON {
            return Some(crate::git_controls::BRANCH_FILTER);
        }
        if (crate::state::COMMENT_FILE_BASE..crate::state::COMMENT_REMOVE_BASE).contains(&number) {
            return Some(crate::state::COMMENT_FIELD);
        }
        (slash_row
            || id == SLASH_BUTTON
            || id == SKILL_BUTTON
            || id == MENTION
            || id == crate::state::REVISE)
            .then_some(crate::state::PROMPT)
    }

    /// Chooses the effort at `index` of those the agent offers.
    fn pick_effort(&mut self, index: usize) {
        let Some(setting) = self
            .settings_now()
            .iter()
            .find(|setting| setting.category == "thought_level")
        else {
            return;
        };
        if let Some(pick) = setting.picks.get(index) {
            let (id, value) = (setting.id.clone(), pick.value.clone());
            self.change(&id, &value);
        }
    }
}

/// Navigation regressions for server-backed file listings.
#[cfg(test)]
mod file_listing_tests {
    use super::*;

    /// An existing conversation requests its skills and retains the server's reply.
    #[test]
    fn existing_session_loads_skills() {
        let mut state = WorkspaceState::sample();
        state.page = Page::Session(0);
        state.skills_of = None;
        state.sessions[0].skills.clear();
        state.outbox.clear();
        let agent = state.sessions[0].agent;
        let project = state.sessions[0].project;
        let directory = state.projects[project]
            .directory()
            .to_string_lossy()
            .into_owned();
        state.sync_context();
        assert!(state.outbox.iter().any(|request| matches!(
            request,
            Outgoing::Skills { agent: asked_agent, project: asked_project, directory: asked_directory, .. }
                if *asked_agent == agent && *asked_project == project && *asked_directory == directory
        )));
        state = state.with_skills(
            agent,
            project,
            vec![Slash {
                name: "review".into(),
                description: "Review changes".into(),
            }],
            Default::default(),
        );
        assert_eq!(state.sessions[0].skills[0].name, "review");
        state.outbox.clear();
        state.sync_context();
        assert!(
            !state
                .outbox
                .iter()
                .any(|request| matches!(request, Outgoing::Skills { .. }))
        );
    }

    /// Returning to a directory restores its files while requesting fresh server data.
    #[test]
    fn restores_listing_and_keeps_late_replies_separate() {
        let mut state = WorkspaceState::sample();
        state.refresh_files();
        let first = state.files_of.clone().unwrap();
        state = state.with_files(
            &first,
            vec!["a.rs".into()],
            vec!["empty".into()],
            vec!["a.rs".into()],
        );
        let directory = state.sessions[0].directory.clone();
        state.sessions[0].directory = "/different-checkout".into();
        state.refresh_files();
        let second = state.files_of.clone().unwrap();
        assert!(state.files.is_empty());
        assert!(state.tree_folders.is_empty());
        assert!(state.tree_ignored.is_empty());
        assert!(state.files_loading());
        state = state.with_files(
            &first,
            vec!["updated.rs".into()],
            vec!["empty".into()],
            vec!["updated.rs".into()],
        );
        assert!(state.files.is_empty());
        state = state.with_files(&second, Vec::new(), Vec::new(), Vec::new());
        assert!(!state.files_loading());
        state.sessions[0].directory = directory;
        state.outbox.clear();
        state.refresh_files();
        assert_eq!(state.files, ["updated.rs"]);
        assert_eq!(state.tree_folders, ["empty"]);
        assert!(state.tree_ignored.contains("updated.rs"));
        assert!(!state.files_loading());
        assert!(
            state
                .outbox
                .iter()
                .any(|request| matches!(request, Outgoing::Files { key, .. } if key == &first))
        );
    }
}

/// Regression coverage for discovery arriving after a conversation has opened.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::Session;
    use crate::state::{SEND, SKILL_BUTTON};

    /// A restored Codex conversation whose skills have not been discovered yet.
    fn conversation() -> WorkspaceState {
        let mut state = WorkspaceState::sample();
        let agent = state
            .offered_agents()
            .into_iter()
            .find(|agent| state.agent_has_skills(*agent))
            .unwrap();
        state.sessions[0] = Session::restored("Task", 0, "task", "/project", None, agent, false);
        state.skills_of = None;
        state.skills.clear();
        state.outbox.clear();
        state
    }

    /// The skill offered by the server in these conversations.
    fn skills() -> Vec<Slash> {
        vec![Slash {
            name: "review".into(),
            description: "Review changes".into(),
        }]
    }

    /// Restoring a conversation requests skills and makes the dollar button show them.
    #[test]
    fn restored_conversation_discovers_skills() {
        let mut state = conversation();
        let agent = state.sessions[0].agent;
        state.sync_context();
        assert!(state.outbox.iter().any(|request| matches!(request, Outgoing::Skills { agent: found, project: 0, .. } if *found == agent)));
        state = state.with_skills(agent, 0, skills(), Default::default());
        state.press_choice(SKILL_BUTTON.0);
        assert_eq!(state.slash_matches(), vec![('$', skills().remove(0))]);
    }

    /// Discovery that finishes after the first message still reaches the conversation.
    #[test]
    fn late_discovery_populates_existing_conversation() {
        let mut state = conversation();
        let agent = state.sessions[0].agent;
        state.skills_of = Some((agent, 0));
        state.page = Page::New(0);
        state.agent = agent;
        state.prompt = prompt_field("Review these changes");
        state = state.after(SEND);
        assert!(state.open().unwrap().skills.is_empty());
        state = state.with_skills(agent, 0, skills(), Default::default());
        assert!(!state.open().unwrap().messages.is_empty());
        state.press_choice(SKILL_BUTTON.0);
        assert_eq!(state.slash_matches().len(), 1);
    }

    /// A conversation restored after discovery reuses the matching cache without another request.
    #[test]
    fn restored_conversation_reuses_cached_skills() {
        let mut state = conversation();
        state.skills_of = Some((state.sessions[0].agent, 0));
        state.skills = skills();
        state.sync_context();
        assert!(
            !state
                .outbox
                .iter()
                .any(|request| matches!(request, Outgoing::Skills { .. }))
        );
        state.press_choice(SKILL_BUTTON.0);
        assert_eq!(state.slash_matches().len(), 1);
    }
}

//! What the reader picks around the prompt box: the agent's settings, a command behind a slash, and where new work is done.

use pm_acp::{Discovery, Setting, Slash};
use pm_input::FocusId;

use crate::state::{
    ACCOUNT, ACCOUNT_BASE, AGENT_TOGGLE, APPROVALS_CHIP, ATTACHED_BASE, Activity, BRANCH_BASE,
    CONTEXT, DETAILS_BASE, EFFORT_BASE, LOCATION, Location, MENTION, MENU_BODY, Outgoing,
    PICK_BASE, PLACE_BASE, PLAN_CHIP, Page, Picker, SETTING_BASE, SHELLS_CHIP, SKILL_BUTTON,
    SLASH_BASE, SLASH_BUTTON, STEP_BASE, SUBAGENTS_CHIP, WorkspaceState, prompt_field,
};

/// How tall one value of a menu is, in logical pixels.
pub const ROW_HEIGHT: f32 = 47.0;

/// The height in logical pixels of one row of the list of files.
const MENTION_ROW_HEIGHT: f32 = 34.0;

/// The categories of settings shown as buttons in the prompt box, in the order they are shown.
const SHOWN: [&str; 2] = ["model", "mode"];

impl WorkspaceState {
    /// Whether pressing `action` leaves the open menu open or works it.
    pub fn keeps_picker(action: FocusId) -> bool {
        let FocusId(id) = action;
        action == LOCATION
            || action == MENU_BODY
            || action == ACCOUNT
            || action == CONTEXT
            || (ACCOUNT_BASE..ACCOUNT_BASE + 50).contains(&id)
            || (SETTING_BASE..SETTING_BASE + 10).contains(&id)
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
    fn agent_now(&self) -> usize {
        self.open().map_or(self.agent, |session| session.agent)
    }

    /// Whether the agent the prompt is for offers skills behind a dollar sign.
    pub fn has_skills(&self) -> bool {
        self.agent_has_skills(self.agent_now())
    }

    /// Who the chosen agent is signed in as: the name of the login in use, or the one in its own default folder.
    pub fn account(&self) -> Option<&str> {
        match self.login_in_use() {
            Some(used) => self
                .logins_here()
                .into_iter()
                .find(|(id, _)| *id == used)
                .map(|(_, name)| name),
            None => self.default_account(),
        }
    }

    /// Who the chosen agent is signed in as in its own default folder, when its files say.
    pub fn default_account(&self) -> Option<&str> {
        self.defaults.get(self.agent)?.as_ref()?.as_deref()
    }

    /// Whether the command named `name` is a skill the agent has installed.
    pub fn is_skill(&self, name: &str) -> bool {
        self.skill_names
            .contains(name.rsplit(':').next().unwrap_or(name))
    }

    /// The commands or skills offered behind a slash or a dollar sign that fit what has been typed, with which it was.
    pub fn slash_matches(&self) -> Vec<(char, &Slash)> {
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
        source
            .iter()
            .filter(|command| command.name.to_lowercase().starts_with(&rest))
            .map(|command| (prefix, command))
            .collect()
    }

    /// The machine and folder the prompt is for: the open session's, or the project being started in.
    fn prompt_place(&self) -> Option<(Option<String>, String)> {
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
        self.files = Vec::new();
        self.files_of = key.clone();
        if let (Some((host, directory)), Some(key)) = (wanted, key) {
            self.outbox.push(Outgoing::Files {
                key,
                host,
                directory,
            });
        }
    }

    /// The state holding `files` as those listed for `key`, unless the prompt has moved on.
    pub fn with_files(mut self, key: &str, files: Vec<String>) -> Self {
        if self.files_of.as_deref() == Some(key) {
            self.files = files;
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

    /// The state holding the skills found for `agent` in the project at `project`, unless the page has moved on.
    pub fn with_skills(
        mut self,
        agent: usize,
        project: usize,
        skills: Vec<pm_acp::Slash>,
        names: std::collections::HashSet<String>,
    ) -> Self {
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

    /// The state knowing who the agent at `agent` is signed in as, and naming the login in use after it when `naming`.
    pub fn with_identity(
        mut self,
        agent: usize,
        default: Option<String>,
        selected: Option<String>,
        naming: bool,
    ) -> Self {
        if let Some(held) = self.defaults.get_mut(agent) {
            *held = Some(default);
        }
        if let Some(held) = self.identities.get_mut(agent) {
            *held = Some(selected.clone());
        }
        if naming {
            self.name_login(agent, selected);
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
        let chosen = self
            .slash_matches()
            .get(index)
            .map(|(prefix, command)| format!("{prefix}{} ", command.name));
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

    /// Reads the branches of the project being started in when it is not the one they were read for.
    pub fn sync_context(&mut self) {
        let Page::New(project) = self.page else {
            return;
        };
        if let Some(held) = self.projects.get(project) {
            self.sync_discovery(held.host());
        }
        if self.skills_of != Some((self.agent, project))
            && let Some(held) = self.projects.get(project)
        {
            self.skills_of = Some((self.agent, project));
            self.skills = Vec::new();
            self.skill_names = Default::default();
            self.outbox.push(Outgoing::Skills {
                agent: self.agent,
                project,
                host: held.host(),
                directory: held.directory().to_string_lossy().into_owned(),
            });
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
                naming: false,
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
        } else if id == CONTEXT.0 {
            self.picker = (self.picker != Some(Picker::Context)).then_some(Picker::Context);
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
        } else if let Some(index) = within(PLACE_BASE, 2) {
            self.location = if index == 0 {
                Location::Worktree
            } else {
                Location::Local
            };
            self.picker = None;
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

    /// The agent's id the account menu is for.
    fn agent_id(&self) -> Option<String> {
        self.agent_id_at(self.agent)
    }

    /// The named logins of the chosen agent: the folder name and what each is called.
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

    /// The folder name of the login in use for the chosen agent, if it is not the agent's own.
    pub fn login_in_use(&self) -> Option<&str> {
        let agent = self.agent_id()?;
        self.in_use
            .iter()
            .find(|(owner, _)| *owner == agent)
            .map(|(_, id)| id.as_str())
    }

    /// Takes the row `index` of the account menu: the agent's own login, a named one, or adding one.
    fn pick_account(&mut self, index: usize) {
        let Some(agent) = self.agent_id() else {
            return;
        };
        let logins: Vec<String> = self
            .logins_here()
            .into_iter()
            .map(|(id, _)| id.to_owned())
            .collect();
        self.in_use.retain(|(owner, _)| *owner != agent);
        if index == 0 {
        } else if let Some(id) = logins.get(index - 1) {
            self.in_use.push((agent.to_owned(), id.clone()));
        } else {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_millis());
            let id = format!("{stamp:x}");
            let name = format!("Account {}", logins.len() + 1);
            self.logins.push((agent.to_owned(), id.clone(), name));
            self.in_use.push((agent.to_owned(), id));
            self.outbox.push(Outgoing::Accounts {
                logins: self.logins.clone(),
                in_use: self.in_use.clone(),
            });
            self.outbox.push(Outgoing::SignIn { agent: self.agent });
            self.identities[self.agent] = None;
            self.picker = None;
            return;
        }
        self.identities[self.agent] = None;
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
        self.picker = None;
    }

    /// Adds the sessions the server of `host` holds that belong to the projects here, and returns each one's position and the server's number for it.
    pub fn restore_sessions(
        &mut self,
        host: &Option<String>,
        infos: &[pm_protocol::SessionInfo],
    ) -> Vec<(usize, u64)> {
        let mut added = Vec::new();
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

    /// Names the login in use for the agent after who it is now signed in as, when it still has the name it was given on adding.
    pub fn name_login(&mut self, agent: usize, who: Option<String>) {
        let Some(held) = self.agent_id_at(agent) else {
            return;
        };
        let Some(who) = who else {
            return;
        };
        let Some((_, used)) = self
            .in_use
            .iter()
            .find(|(owner, _)| *owner == held)
            .cloned()
        else {
            return;
        };
        if let Some(login) = self
            .logins
            .iter_mut()
            .find(|(owner, id, name)| *owner == held && *id == used && name.starts_with("Account "))
        {
            login.2 = who;
            self.outbox.push(Outgoing::Accounts {
                logins: self.logins.clone(),
                in_use: self.in_use.clone(),
            });
        }
    }

    /// Asks who the agent is signed in as, to name the login in use after it.
    pub fn outbox_identity(&mut self, agent: usize) {
        self.outbox.push(Outgoing::Identity {
            agent,
            naming: true,
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

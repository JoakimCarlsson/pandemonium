//! The lists the quick input offers to pick one thing from: a theme, a keymap, a setting of the agent and its values, a message to rewind to or fork from, a branch to check out, or what to type to list something else; and what picking each does.

use pm_editor::Preset;
use pm_input::FocusId;
use pm_text::Field;

use crate::listing::{Items, Listing};
use crate::state::{FORK_BASE, Outgoing, Page, RESTORE_BASE, REWIND_BASE, WorkspaceState};

/// What a list of choices is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Choosing {
    /// The theme the window is drawn in.
    Theme,
    /// The keymap the keys start from.
    Keymap,
    /// Which setting of the agent to change.
    AgentSetting,
    /// A value of the setting of the agent at this position.
    SettingValue(usize),
    /// A message of the reader's to take the conversation back to before.
    Rewind,
    /// A message of the reader's to put the files back as they were before.
    Restore,
    /// A reply of the agent's to start a session branching off after.
    Fork,
    /// A branch to check out.
    Branch,
    /// What to type to list something else.
    Help,
}

impl Choosing {
    /// What the field says while nothing is typed in it.
    pub const fn placeholder(self) -> &'static str {
        match self {
            Self::Theme => "Select a theme",
            Self::Keymap => "Select a keymap",
            Self::AgentSetting => "Select a setting of the agent",
            Self::SettingValue(_) => "Select a value",
            Self::Rewind => "Select a message to rewind the conversation to before",
            Self::Restore => "Select a message to restore the files to before",
            Self::Fork => "Select a reply to fork the conversation after",
            Self::Branch => "Select a branch to check out",
            Self::Help => "Type a prefix, or select one",
        }
    }

    /// What the list says when nothing is left to pick.
    pub const fn empty(self) -> &'static str {
        match self {
            Self::Theme | Self::Keymap | Self::Help => "Nothing matches",
            Self::AgentSetting | Self::SettingValue(_) => "The agent has nothing to change here",
            Self::Rewind | Self::Restore => "No messages of yours in this conversation",
            Self::Fork => "No replies of the agent's in this conversation",
            Self::Branch => "No branches",
        }
    }
}

/// What picking a choice does.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    /// Presses the control `id`, as a click on it would.
    Press(FocusId),
    /// Draws the window in the theme family of this name.
    Theme(String),
    /// Starts the keys from this keymap.
    Keymap(Preset),
    /// Lists the values of the setting of the agent at this position.
    Setting(usize),
    /// Gives the setting of the agent at this position this value.
    Value(usize, String),
    /// Checks out the branch of this name.
    Checkout(String),
    /// Types this in the quick input, to list what it says.
    Type(&'static str),
}

/// One thing to pick.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// What it is called.
    pub label: String,
    /// What is said beside it.
    pub detail: String,
    /// Whether it is what holds now.
    pub current: bool,
    /// What picking it does.
    pub then: Then,
}

impl Choice {
    /// A choice called `label`, with `detail` beside it, doing `then`.
    fn new(label: impl Into<String>, detail: impl Into<String>, then: Then) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            current: false,
            then,
        }
    }

    /// The choice marked as what holds now when `current`.
    fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }
}

/// What the quick input lists after each prefix, as the help lists them.
const PREFIXES: [(&str, &str); 7] = [
    ("", "Go to a file"),
    (">", "Run a command"),
    ("#", "Go to a symbol in the project"),
    ("@", "Go to a symbol in the file"),
    ("%", "Search the text of the project"),
    (":", "Go to a line"),
    ("?", "List what can be typed first"),
];

/// The first line of `text`, as a list names a message by it.
fn first_line(text: &str) -> String {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_owned()
}

impl WorkspaceState {
    /// The choices of `choosing` as the workspace stands.
    pub(crate) fn choices_of(&self, choosing: Choosing) -> Vec<Choice> {
        match choosing {
            Choosing::Theme => {
                let held = &self.window.appearance.dark_theme;
                pm_theme::families()
                    .into_iter()
                    .map(|family| {
                        Choice::new(family.name, "", Then::Theme(family.name.to_owned()))
                            .current(family.name == held)
                    })
                    .collect()
            }
            Choosing::Keymap => Preset::ALL
                .into_iter()
                .map(|preset| {
                    Choice::new(preset.title(), "", Then::Keymap(preset))
                        .current(preset == self.editor.keymap)
                })
                .collect(),
            Choosing::AgentSetting => self
                .settings_now()
                .iter()
                .enumerate()
                .map(|(index, setting)| {
                    let now = setting.current_name().unwrap_or(&setting.current);
                    Choice::new(setting.name.clone(), now, Then::Setting(index))
                })
                .collect(),
            Choosing::SettingValue(index) => self
                .settings_now()
                .get(index)
                .map(|setting| {
                    setting
                        .picks
                        .iter()
                        .map(|pick| {
                            Choice::new(
                                pick.name.clone(),
                                pick.description.clone(),
                                Then::Value(index, pick.value.clone()),
                            )
                            .current(pick.value == setting.current)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Choosing::Rewind | Choosing::Restore | Choosing::Fork => {
                let Some(session) = self.open() else {
                    return Vec::new();
                };
                let (wanted, base) = match choosing {
                    Choosing::Rewind => (true, REWIND_BASE),
                    Choosing::Restore => (true, RESTORE_BASE),
                    _ => (false, FORK_BASE),
                };
                session
                    .messages
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, message)| message.from_user == wanted)
                    .map(|(index, message)| {
                        Choice::new(
                            first_line(&message.text),
                            "",
                            Then::Press(FocusId(base + index as u64)),
                        )
                    })
                    .filter(|choice| !choice.label.is_empty())
                    .collect()
            }
            Choosing::Branch => self
                .git
                .branches
                .iter()
                .enumerate()
                .map(|(place, name)| {
                    Choice::new(name.clone(), "", Then::Checkout(name.clone())).current(place == 0)
                })
                .collect(),
            Choosing::Help => PREFIXES
                .into_iter()
                .map(|(prefix, said)| {
                    let typed = if prefix.is_empty() { "…" } else { prefix };
                    Choice::new(typed, said, Then::Type(prefix))
                })
                .collect(),
        }
    }

    /// Opens the quick input at `at` over the choices of `choosing`, the one that holds now picked.
    pub fn choices_requested(mut self, choosing: Choosing, at: (f32, f32)) -> Self {
        if choosing == Choosing::Branch
            && let Some(session) = self.review_index()
        {
            self.outbox.push(Outgoing::SessionBranches { session });
        }
        let filter = Field::new(if choosing == Choosing::Help { "?" } else { "" });
        let choices = self.choices_of(choosing);
        let matched = crate::listing::matching_choices(&choices, "");
        let picked = matched
            .iter()
            .position(|place| choices[*place].current)
            .unwrap_or(0);
        let problem = choices.is_empty().then(|| choosing.empty().to_owned());
        self.listing = Some(Listing {
            session: self.review_index().unwrap_or_default(),
            items: Items::Choices {
                choosing,
                choices,
                filter,
                matched,
            },
            picked,
            answered: true,
            problem,
            at,
            quick: true,
        });
        self
    }

    /// The choices of the list open over the window renewed from the workspace, when it lists `choosing`, keeping what is typed.
    pub(crate) fn choices_renewed(&mut self, choosing: Choosing) {
        let renewed = self.choices_of(choosing);
        if let Some(listing) = self.listing.as_mut()
            && let Items::Choices {
                choosing: held,
                choices,
                filter,
                matched,
            } = &mut listing.items
            && *held == choosing
        {
            *matched = crate::listing::matching_choices(&renewed, filter.text());
            *choices = renewed;
            listing.problem = choices.is_empty().then(|| choosing.empty().to_owned());
        }
    }

    /// The state after `choice` was picked from the quick input hanging at `at`.
    pub(crate) fn choice_taken(mut self, choice: Choice, at: (f32, f32)) -> Self {
        match choice.then {
            Then::Press(id) => self.after(id),
            Then::Theme(name) => {
                self.window.appearance.light_theme = name.clone();
                self.window.appearance.dark_theme = name;
                self
            }
            Then::Keymap(preset) => {
                self.editor.keymap = preset;
                self
            }
            Then::Setting(index) => self.choices_requested(Choosing::SettingValue(index), at),
            Then::Value(index, value) => {
                self.agent_setting_changed(index, &value);
                self
            }
            Then::Checkout(branch) => {
                if let Some(session) = self.review_index() {
                    self.git.trouble = None;
                    self.git.busy = Some("Switching branch…");
                    self.outbox.push(Outgoing::Checkout { session, branch });
                }
                self
            }
            Then::Type(prefix) => self.quick_typed(prefix, at),
        }
    }

    /// Opens the quick input at `at` over the values of the agent's setting of `category`, or says the agent has none.
    pub fn agent_category_requested(self, category: &str, at: (f32, f32)) -> Self {
        let index = self
            .settings_now()
            .iter()
            .position(|setting| setting.category == category);
        match index {
            Some(index) => self.choices_requested(Choosing::SettingValue(index), at),
            None => self.choices_requested(Choosing::AgentSetting, at),
        }
    }

    /// The state with the menu of the session shown pressed on its row `row`, as a click on that row would.
    pub fn session_command(mut self, row: FocusId) -> Self {
        let Page::Session(session) = self.page else {
            return self;
        };
        self.session_menu = Some(crate::arrange::SessionMenu {
            session,
            at: (0.0, 0.0),
        });
        self.after(row)
    }

    /// The control answering what the agent of the session shown asks with an answer of `verdict`, when it asks.
    pub fn answer_of(&self, verdict: pm_acp::Verdict) -> Option<FocusId> {
        let ask = self.open()?.ask.as_ref()?;
        let index = ask
            .choices
            .iter()
            .position(|choice| choice.verdict == verdict)?;
        Some(FocusId(crate::state::CHOICE_BASE + index as u64))
    }

    /// The state with the menu of the tab shown pressed on `command`, as a click on that row would.
    pub fn shown_tab_command(self, command: crate::tab_menu::TabCommand) -> Self {
        match self.git.shown {
            Some(index) => self.with_tab_menu(index, (0.0, 0.0)).tab_command(command),
            None => self,
        }
    }

    /// The state working in the pane `delta` panes along from the one worked in, wrapping around.
    pub fn pane_stepped(mut self, delta: isize) -> Self {
        let panes = self.panes();
        let count = panes.layout.groups().len();
        if count > 1 {
            let next = (panes.focused as isize + delta).rem_euclid(count as isize) as usize;
            self.focus_pane(next);
        }
        self
    }

    /// The state drawn in `mode`: light, dark, or as the desktop says.
    pub fn with_theme_mode(mut self, mode: pm_config::ThemeMode) -> Self {
        self.window.appearance.theme_mode = mode;
        self
    }

    /// The state with the text of the window and of files `steps` points bigger, smaller when negative, or at their usual sizes when `steps` is nothing.
    pub fn zoomed(mut self, steps: Option<i32>) -> Self {
        let appearance = &mut self.window.appearance;
        match steps {
            Some(steps) => {
                appearance.interface_size =
                    (appearance.interface_size as i32 + steps).clamp(10, 20) as u32;
                self.editor.font_size = (self.editor.font_size as i32 + steps).clamp(9, 28) as u32;
            }
            None => {
                appearance.interface_size = pm_config::AppearanceSettings::default().interface_size;
                self.editor.font_size = pm_editor::EditorSettings::default().font_size;
            }
        }
        self
    }
}

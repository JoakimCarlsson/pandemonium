//! The onboarding flow: what the app is for, a project to start with and the
//! agents that are ready to work in it.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, flex, flex_1, flex_col, font_medium, font_semibold, gap, items_center,
    justify_center, leading_ratio, max_w, min_h, min_w_px, p, rounded, size, text_color, text_px,
    tracking_tight, tracking_wide, w_full,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node, text};

use pm_components::Button;
use pm_components::Input;
use pm_components::Size;
use pm_components::button::Variant;
use pm_components::context_menu::{ContextMenu, field_menu};
use pm_components::extensions::{endings, pickable, standing};
use pm_components::folder_picker::{FolderPicker, Outcome, PICKER_FIELD, Purpose, is_picker};
use pm_components::parts::{glyph, line};
use pm_components::project::Project;
use pm_components::state::Outgoing;
use pm_components::{Avatar, Checkbox, Progress, Tile, TitleBar};
use pm_protocol::ExtensionInfo;

/// Moves to the step after the current one.
pub const CONTINUE: FocusId = FocusId(100);

/// Moves to the step before the current one.
pub const BACK: FocusId = FocusId(101);

/// Moves past the project step without adding a project.
pub const SKIP: FocusId = FocusId(102);

/// Adds the project the path field names.
pub const ADD: FocusId = FocusId(103);

/// Opens a folder picker to choose a project from.
pub const BROWSE: FocusId = FocusId(105);

/// Leaves onboarding for the workspace.
pub const FINISH: FocusId = FocusId(104);

/// The folder or repository address field.
pub const PATH: FocusId = FocusId(110);

/// The first of the buttons removing an added project; the rest follow it.
const REMOVE_BASE: u64 = 140;

/// The first of the buttons moving an agent along; the rest follow it.
const AGENT_BASE: u64 = 160;

/// The first of the rows ticking a language to install; the rest follow it.
const LANGUAGE_PICK_BASE: u64 = 600;

/// One past the last of the rows ticking a language.
const LANGUAGE_PICK_END: u64 = 700;

/// How many characters of a failure an agent row has room for.
const REASON_WIDTH: usize = 72;

/// The three stages of working with an agent, in the order the app puts them.
const FLOW: [(IconName, &str, &str); 3] = [
    (
        IconName::Sparkles,
        "Ask an agent",
        "Describe the work. It runs in its own worktree.",
    ),
    (
        IconName::GitCompare,
        "Review what changed",
        "Read the diff, comment on lines, send it back.",
    ),
    (
        IconName::FileCode,
        "Edit code last",
        "Open a file only when you need to touch it yourself.",
    ),
];

/// Where an agent stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentStatus {
    /// Being started to find out.
    Checking,
    /// Being installed or signed in.
    Working,
    /// Started and signed in.
    Ready,
    /// Started, wants a login.
    SignedOut,
    /// Not installed.
    Missing,
    /// Started but did not work, for this reason.
    Failed(String),
}

/// A step of onboarding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step {
    /// What the app is for.
    Welcome,
    /// Adding a project to start with.
    Project,
    /// Choosing the languages to write code in.
    Languages,
    /// Choosing which agents are ready.
    Agents,
}

impl Step {
    /// Every step, in the order onboarding shows them.
    const ALL: [Self; 4] = [Self::Welcome, Self::Project, Self::Languages, Self::Agents];

    /// The step after this one, or this one when it is the last.
    fn next(self) -> Self {
        match self {
            Self::Welcome => Self::Project,
            Self::Project => Self::Languages,
            Self::Languages | Self::Agents => Self::Agents,
        }
    }

    /// The step before this one, or this one when it is the first.
    fn previous(self) -> Self {
        match self {
            Self::Welcome | Self::Project => Self::Welcome,
            Self::Languages => Self::Project,
            Self::Agents => Self::Languages,
        }
    }
}

/// What onboarding has collected so far.
#[derive(Clone, Debug, PartialEq)]
pub struct OnboardingState {
    /// The step on screen.
    pub step: Step,
    /// The address typed for the next project.
    pub path: Field,
    /// The projects added, in the order they were added.
    pub projects: Vec<Project>,
    /// Where each of the agents stands.
    pub agents: Vec<AgentStatus>,
    /// The glyph and name of each of them, in the order the server lists them.
    pub listed: Vec<(IconName, String)>,
    /// Whether the reader has left onboarding.
    pub finished: bool,
    /// The menu open over a field, and where, if one is.
    pub menu: Option<ContextMenu>,
    /// The languages the server on this machine can install, once it has said.
    pub languages: Vec<ExtensionInfo>,
    /// The languages ticked to install, by folder name.
    pub chosen: Vec<String>,
    /// What the server has yet to be asked.
    pub outbox: Vec<Outgoing>,
    /// The window for choosing a folder, if it is open.
    pub folder_picker: Option<FolderPicker>,
}

impl Default for OnboardingState {
    /// The welcome step, no projects and the agents as the first launch finds them.
    fn default() -> Self {
        Self {
            step: Step::Welcome,
            path: Field::default(),
            projects: Vec::new(),
            agents: Vec::new(),
            listed: Vec::new(),
            finished: false,
            menu: None,
            languages: Vec::new(),
            chosen: Vec::new(),
            outbox: Vec::new(),
            folder_picker: None,
        }
        .listing(&pm_protocol::builtin_agents())
    }
}

impl OnboardingState {
    /// The state listing `agents` to connect, each still being checked.
    pub fn listing(mut self, agents: &[pm_protocol::AgentServer]) -> Self {
        self.listed = agents
            .iter()
            .map(|agent| {
                (
                    pm_components::agent_list::agent_icon(agent),
                    agent.name.clone(),
                )
            })
            .collect();
        self.agents = vec![AgentStatus::Checking; agents.len()];
        self
    }

    /// The state after `action` was pressed.
    pub fn after(mut self, action: FocusId) -> Self {
        match action {
            CONTINUE if self.step == Step::Languages => {
                if !self.chosen.is_empty() {
                    self.outbox.push(Outgoing::InstallExtensions {
                        ids: self.chosen.clone(),
                    });
                }
                self.step = self.step.next();
            }
            CONTINUE => {
                self.step = self.step.next();
                if self.step == Step::Languages {
                    self.outbox.push(Outgoing::Extensions);
                }
            }
            BACK => self.step = self.step.previous(),
            SKIP => {
                self.step = Step::Languages;
                self.outbox.push(Outgoing::Extensions);
            }
            FocusId(id) if (LANGUAGE_PICK_BASE..LANGUAGE_PICK_END).contains(&id) => {
                let place = (id - LANGUAGE_PICK_BASE) as usize;
                if let Some(language) = pickable(&self.languages).nth(place) {
                    let id = language.id.clone();
                    match self.chosen.iter().position(|held| *held == id) {
                        Some(at) => {
                            self.chosen.remove(at);
                        }
                        None => self.chosen.push(id),
                    }
                }
            }
            FINISH if self.has_ready_agent() => self.finished = true,
            BROWSE => {
                let (picker, ask) = FolderPicker::opened(Purpose::Open, None);
                self.folder_picker = Some(picker);
                self.outbox.push(ask);
            }
            action if is_picker(action) => return self.folder_picker_pressed(action),
            ADD => {
                let source = self.path.text().to_owned();
                self.add(&source);
                self.path = Field::default();
            }
            FocusId(id) if (REMOVE_BASE..AGENT_BASE).contains(&id) => {
                let index = (id - REMOVE_BASE) as usize;
                if index < self.projects.len() {
                    self.projects.remove(index);
                }
            }
            _ => {}
        }
        self
    }

    /// The state after `edit` is made to the field named `id`.
    pub fn edited(mut self, id: FocusId, edit: Edit) -> Self {
        match (id, self.folder_picker.as_mut()) {
            (PATH, _) => self.path.apply(edit),
            (PICKER_FIELD, Some(open)) => open.edited(edit),
            _ => {}
        }
        self
    }

    /// The state after something of the folder picker was pressed: `action` is the name of what.
    fn folder_picker_pressed(mut self, action: FocusId) -> Self {
        let Some(mut open) = self.folder_picker.take() else {
            return self;
        };
        match open.pressed(action) {
            Outcome::Staying(ask) => {
                self.outbox.extend(ask);
                self.folder_picker = Some(open);
            }
            Outcome::Closed => {}
            Outcome::Chosen(path) => self.add(&open.address(&path)),
        }
        self
    }

    /// The state with the folders the server listed for `requested`, or why it could not, when that is still the folder the picker wants.
    pub fn with_folders(
        mut self,
        requested: &str,
        listed: Result<(String, Vec<String>), String>,
    ) -> Self {
        if let Some(open) = self.folder_picker.as_mut() {
            open.listed(requested, listed);
        }
        self
    }

    /// The state with the list of the folder picker scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_folder_picker(mut self, delta: f32, room: f32) -> Self {
        if let Some(open) = self.folder_picker.as_mut() {
            open.scrolled(delta, room);
        }
        self
    }

    /// The state with the folder picker put away.
    pub fn without_folder_picker(mut self) -> Self {
        self.folder_picker = None;
        self
    }

    /// The state after the folder at `path` is chosen as a project.
    pub fn with_project(mut self, path: &str) -> Self {
        self.add(path);
        self
    }

    /// Which agent the button `id` acts on, if it is one of theirs.
    pub fn agent_of(id: FocusId) -> Option<usize> {
        let FocusId(id) = id;
        (AGENT_BASE..AGENT_BASE + 100)
            .contains(&id)
            .then(|| (id - AGENT_BASE) as usize)
    }

    /// The state with the agent at `index` standing at `status`.
    pub fn with_agent(mut self, index: usize, status: AgentStatus) -> Self {
        if let Some(slot) = self.agents.get_mut(index) {
            *slot = status;
        }
        self
    }

    /// The text field `id` names, if it names one.
    pub fn field(&self, id: FocusId) -> Option<&Field> {
        match id {
            PATH => Some(&self.path),
            PICKER_FIELD => self.folder_picker.as_ref().map(|open| &open.field),
            _ => None,
        }
    }

    /// The state with a menu open over the field `field` at `at`.
    pub fn with_menu(mut self, field: FocusId, at: (f32, f32)) -> Self {
        self.menu = Some(ContextMenu { field, at });
        self
    }

    /// The state with no menu open.
    pub fn without_menu(mut self) -> Self {
        self.menu = None;
        self
    }

    /// Whether `id` names a text field.
    pub fn is_field(id: FocusId) -> bool {
        id == PATH || id == PICKER_FIELD
    }

    /// The state after the server on this machine said which languages there are; the ones a project's files are written in start ticked the first time.
    pub fn with_extensions(mut self, languages: Vec<ExtensionInfo>) -> Self {
        if self.languages.is_empty() && self.chosen.is_empty() {
            self.chosen = languages
                .iter()
                .filter(|language| language.installed && !language.hidden)
                .map(|language| language.id.clone())
                .collect();
        }
        self.languages = languages;
        self
    }

    /// What the server has yet to be asked, taken off the state.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.outbox)
    }

    /// Whether at least one agent can be used.
    fn has_ready_agent(&self) -> bool {
        self.agents.contains(&AgentStatus::Ready)
    }

    /// Adds a project named after `source`, unless it is empty or already added.
    fn add(&mut self, source: &str) {
        if let Some(project) = Project::from_source(source)
            && !self.projects.iter().any(|held| held.path == project.path)
        {
            self.projects.push(project);
        }
    }
}

/// A heading of `title` at `pixels` tall type above `lines` of body text at
/// `body` tall type, each its own line since text does not wrap.
fn heading(
    title: &str,
    pixels: u32,
    ratio: f32,
    body: u32,
    body_ratio: f32,
    lines: &[&str],
) -> Node {
    node()
        .styles([flex(), flex_col(), gap(if pixels > 30 { 3 } else { 2 })])
        .child(text(title).styles([
            text_px(pixels),
            leading_ratio(ratio),
            font_semibold(),
            tracking_tight(),
            text_color(Role::Text),
        ]))
        .child(
            node().styles([flex(), flex_col()]).children(
                lines
                    .iter()
                    .map(|content| line(*content, body, body_ratio, Role::Muted)),
            ),
        )
}

/// The bar showing how far through onboarding the reader is.
fn progress(step: Step) -> Node {
    Progress::new(Step::ALL.len(), step as usize).build()
}

/// The welcome step: what the app is for.
fn welcome() -> Node {
    node()
        .styles([flex(), flex_col(), gap(8), w_full()])
        .child(heading(
            "Welcome. Let's set you up.",
            34,
            1.15,
            15,
            1.6,
            &[
                "Pandemonium puts agents first. You hand over the work, review what",
                "changed, and only then open the code. Two quick steps and you're in.",
            ],
        ))
        .child(
            node()
                .styles([flex(), flex_col(), gap(3), w_full()])
                .children(
                    FLOW.iter()
                        .enumerate()
                        .map(|(index, (icon, title, detail))| {
                            Tile::strip(18).build().children([
                                node()
                                    .styles([
                                        flex(),
                                        items_center(),
                                        justify_center(),
                                        size(10),
                                        rounded(Radius::Lg),
                                        bg(Role::SurfaceSelected),
                                    ])
                                    .child(glyph(*icon, 5.0, Role::Text)),
                                node().styles([flex(), flex_col(), flex_1()]).children([
                                    text(format!("{}. {}", index + 1, title)).styles([
                                        text_px(14),
                                        leading_ratio(1.4),
                                        font_medium(),
                                        text_color(Role::Text),
                                    ]),
                                    line(*detail, 13, 1.4, Role::Muted),
                                ]),
                            ])
                        }),
                ),
        )
}

/// The project step: a field for a folder or repository, one-click folders and what is added so far.
fn project(state: &OnboardingState) -> Node {
    let can_add = Project::from_source(state.path.text()).is_some();
    let added = state.projects.iter().enumerate().map(|(index, project)| {
        Tile::strip(11).build().children([
            Avatar::of(project).build(),
            node()
                .styles([flex_1()])
                .child(line(project.name.clone(), 13, 1.4, Role::Text)),
            Button::icon(IconName::Close, FocusId(REMOVE_BASE + index as u64)).build(),
        ])
    });
    node()
        .styles([flex(), flex_col(), gap(4), w_full()])
        .child(heading(
            "Add a project",
            28,
            1.25,
            14,
            1.4,
            &[
                "A project is a folder or a Git repository.",
                "Each task you start gets its own worktree of it.",
            ],
        ))
        .child(
            node()
                .styles([flex(), gap(2), w_full()])
                .child(
                    node().styles([flex_1(), min_w_px(0)]).child(
                        Input::new(&state.path, PATH)
                            .placeholder("Folder path or Git URL")
                            .build(),
                    ),
                )
                .child(
                    Button::new("Add", ADD)
                        .size(Size::Lg)
                        .disabled(!can_add)
                        .build(),
                )
                .child(
                    Button::new("Browse…", BROWSE)
                        .variant(Variant::Outline)
                        .size(Size::Lg)
                        .build(),
                ),
        )
        .child(
            node()
                .styles([flex(), flex_col(), gap(2), w_full()])
                .children(added),
        )
}

/// The agent step: which agents are ready, and a way to get the others there.
fn agents(state: &OnboardingState) -> Node {
    node()
        .styles([flex(), flex_col(), gap(4), w_full()])
        .child(heading(
            "Connect your agents",
            28,
            1.25,
            14,
            1.4,
            &[
                "Use the agents you already have. You can run any of them in any",
                "project, and add accounts later.",
            ],
        ))
        .child(
            node()
                .styles([flex(), flex_col(), gap(2), w_full()])
                .children(state.listed.iter().zip(&state.agents).enumerate().map(
                    |(index, ((icon, name), status))| {
                        let (detail, action) = match status {
                            AgentStatus::Checking => ("Checking…".to_owned(), None),
                            AgentStatus::Working => ("Working…".to_owned(), None),
                            AgentStatus::Ready => ("Signed in".to_owned(), None),
                            AgentStatus::SignedOut => {
                                ("Installed, signed out".to_owned(), Some("Sign in"))
                            }
                            AgentStatus::Missing => ("Not installed".to_owned(), Some("Install")),
                            AgentStatus::Failed(reason) => {
                                (reason.chars().take(REASON_WIDTH).collect(), Some("Retry"))
                            }
                        };
                        Tile::strip(14).build().children([
                            glyph(*icon, 5.0, Role::Muted),
                            node().styles([flex(), flex_col(), flex_1()]).children([
                                line(name, 14, 1.4, Role::Text),
                                line(detail, 12, 1.4, Role::Subtle),
                            ]),
                            match (action, status) {
                                (Some(label), _) => {
                                    Button::new(label, FocusId(AGENT_BASE + index as u64))
                                        .variant(Variant::Outline)
                                        .size(Size::Sm)
                                        .build()
                                }
                                (None, AgentStatus::Ready) => {
                                    node().styles([flex(), items_center(), gap(1.5)]).children([
                                        glyph(IconName::Check, 4.0, Role::Success),
                                        line("Ready", 12, 1.4, Role::Success),
                                    ])
                                }
                                (None, _) => node(),
                            },
                        ])
                    },
                )),
        )
}

/// The language step: every language there is, ticked to install it on this machine.
fn languages(state: &OnboardingState) -> Node {
    let rows: Vec<Node> = pickable(&state.languages)
        .enumerate()
        .map(|(place, language)| {
            let action = FocusId(LANGUAGE_PICK_BASE + place as u64);
            let ticked = state.chosen.contains(&language.id);
            let (said, tone) = if language.installed || language.busy.is_some() {
                standing(language)
            } else {
                (endings(language), Role::Subtle)
            };
            Tile::new(action).build().children([
                Checkbox::new(ticked, action).build(),
                node().styles([flex(), flex_col(), flex_1()]).children([
                    line(language.name.clone(), 14, 1.4, Role::Text),
                    line(said, 12, 1.4, tone),
                ]),
            ])
        })
        .collect();
    let list = if rows.is_empty() {
        line("Asking which languages there are…", 13, 1.4, Role::Subtle)
    } else {
        node()
            .styles([pm_style::grid(), pm_style::grid_cols(2), gap(2), w_full()])
            .children(rows)
    };
    node()
        .styles([flex(), flex_col(), gap(4), w_full()])
        .child(heading(
            "Pick your languages",
            28,
            1.25,
            14,
            1.4,
            &[
                "Each language brings colours, completions and diagnostics, with its",
                "language server. Nothing is bundled; add more later in Settings.",
            ],
        ))
        .child(list)
}

/// The buttons under a step: back on the left, the way forward on the right.
fn footer(state: &OnboardingState) -> Node {
    let forward = match state.step {
        Step::Welcome => Button::new("Continue", CONTINUE).size(Size::Lg).build(),
        Step::Project => Button::new("Continue", CONTINUE)
            .size(Size::Lg)
            .disabled(state.projects.is_empty())
            .build(),
        Step::Languages if state.chosen.is_empty() => {
            Button::new("Continue", CONTINUE).size(Size::Lg).build()
        }
        Step::Languages => Button::new("Install and continue", CONTINUE)
            .size(Size::Lg)
            .build(),
        Step::Agents => Button::new("Open workspace", FINISH)
            .size(Size::Lg)
            .disabled(!state.has_ready_agent())
            .build(),
    };
    let skip = state.step == Step::Project && state.projects.is_empty();
    node()
        .styles([flex(), items_center(), gap(3), w_full()])
        .children((state.step != Step::Welcome).then(|| {
            Button::new("Back", BACK)
                .variant(Variant::Ghost)
                .size(Size::Lg)
                .build()
        }))
        .child(node().styles([flex_1()]))
        .children(skip.then(|| {
            Button::new("Skip", SKIP)
                .variant(Variant::Ghost)
                .size(Size::Lg)
                .build()
        }))
        .child(forward)
}

/// The whole onboarding screen for `state`, under a title bar holding only the controls of the window.
pub fn onboarding(state: &OnboardingState) -> Node {
    let step = match state.step {
        Step::Welcome => welcome(),
        Step::Project => project(state),
        Step::Languages => languages(state),
        Step::Agents => agents(state),
    };
    let body = node()
        .styles([flex(), flex_col(), gap(8), w_full(), max_w(160)])
        .child(
            node()
                .styles([flex(), items_center(), w_full()])
                .child(text("Pandemonium").styles([
                    text_px(12),
                    leading_ratio(1.4),
                    font_medium(),
                    tracking_wide(),
                    text_color(Role::Muted),
                ]))
                .child(node().styles([flex_1()]))
                .child(progress(state.step)),
        )
        .child(step)
        .child(footer(state));
    let centered = node()
        .styles([
            flex(),
            flex_1(),
            min_h(0),
            items_center(),
            justify_center(),
            w_full(),
            p(10),
        ])
        .child(body);
    node()
        .styles([
            flex(),
            flex_col(),
            w_full(),
            pm_style::h_full(),
            bg(Role::Background),
        ])
        .child(TitleBar::new().build())
        .child(centered)
        .children(state.menu.and_then(|open| menu_over(state, open)))
}

/// The menu for the field it was opened on, if that field still exists and is not the folder picker's, which has a window of its own.
fn menu_over(state: &OnboardingState, open: ContextMenu) -> Option<Node> {
    state
        .field(open.field)
        .filter(|_| !is_picker(open.field))
        .map(|field| field_menu(field, open))
}

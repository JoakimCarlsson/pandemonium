//! The onboarding flow: what the app is for, a project to start with and the
//! agents that are ready to work in it.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_color, flex, flex_1, flex_col, font_medium,
    font_semibold, gap, h, items_center, justify_center, leading_ratio, max_w, min_w_px, p, px,
    rounded, rounded_full, size, text_color, text_px, tracking_tight, tracking_wide, w, w_full,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::{Size, Variant};
use crate::{Input, Menu};

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

/// Takes back the last change to the field the menu was opened on.
pub const MENU_UNDO: FocusId = FocusId(200);

/// Makes again the change that was taken back.
pub const MENU_REDO: FocusId = FocusId(201);

/// Copies the selection and deletes it.
pub const MENU_CUT: FocusId = FocusId(202);

/// Copies the selection.
pub const MENU_COPY: FocusId = FocusId(203);

/// Inserts what the clipboard holds.
pub const MENU_PASTE: FocusId = FocusId(204);

/// Selects the whole line.
pub const MENU_SELECT_ALL: FocusId = FocusId(205);

/// The folder or repository address field.
pub const PATH: FocusId = FocusId(110);

/// The first of the buttons removing an added project; the rest follow it.
const REMOVE_BASE: u64 = 140;

/// The first of the buttons moving an agent along; the rest follow it.
const AGENT_BASE: u64 = 160;

/// How many characters of a failure an agent row has room for.
const REASON_WIDTH: usize = 72;

/// The avatar colours added projects take in turn.
const AVATARS: [Color; 6] = [
    Color::Orange700,
    Color::Violet500,
    Color::Teal600,
    Color::Pink600,
    Color::Sky600,
    Color::Amber600,
];

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

/// The agents onboarding offers, in the order the app lists them.
const AGENTS: [(IconName, &str); 3] = [
    (IconName::Sparkles, "Claude Code"),
    (IconName::Terminal, "Codex"),
    (IconName::Zap, "Grok Build"),
];

/// A step of onboarding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step {
    /// What the app is for.
    Welcome,
    /// Adding a project to start with.
    Project,
    /// Choosing which agents are ready.
    Agents,
}

impl Step {
    /// Every step, in the order onboarding shows them.
    const ALL: [Self; 3] = [Self::Welcome, Self::Project, Self::Agents];

    /// The step after this one, or this one when it is the last.
    fn next(self) -> Self {
        match self {
            Self::Welcome => Self::Project,
            Self::Project | Self::Agents => Self::Agents,
        }
    }

    /// The step before this one, or this one when it is the first.
    fn previous(self) -> Self {
        match self {
            Self::Welcome | Self::Project => Self::Welcome,
            Self::Agents => Self::Project,
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
    /// The names of the projects added, in the order they were added.
    pub projects: Vec<String>,
    /// Where each of the agents stands.
    pub agents: [AgentStatus; 3],
    /// Whether the reader has left onboarding.
    pub finished: bool,
    /// The menu open over a field, and where, if one is.
    pub menu: Option<ContextMenu>,
}

/// A menu open over a text field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenu {
    /// The field it acts on.
    pub field: FocusId,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
}

impl Default for OnboardingState {
    /// The welcome step, no projects and the agents as the first launch finds them.
    fn default() -> Self {
        Self {
            step: Step::Welcome,
            path: Field::default(),
            projects: Vec::new(),
            agents: AGENTS.map(|_| AgentStatus::Checking),
            finished: false,
            menu: None,
        }
    }
}

/// The name a project takes from the folder or repository it was added from.
fn name_of(source: &str) -> &str {
    let trimmed = source.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

impl OnboardingState {
    /// The state after `action` was pressed.
    pub fn after(mut self, action: FocusId) -> Self {
        match action {
            CONTINUE => self.step = self.step.next(),
            BACK => self.step = self.step.previous(),
            SKIP => self.step = Step::Agents,
            FINISH if self.has_ready_agent() => self.finished = true,
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
        if id == PATH {
            self.path.apply(edit);
        }
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
        (id >= AGENT_BASE && id < AGENT_BASE + AGENTS.len() as u64)
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
        (id == PATH).then_some(&self.path)
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

    /// Whether `id` names a row of the menu.
    pub fn is_menu_item(id: FocusId) -> bool {
        [
            MENU_UNDO,
            MENU_REDO,
            MENU_CUT,
            MENU_COPY,
            MENU_PASTE,
            MENU_SELECT_ALL,
        ]
        .contains(&id)
    }

    /// Whether `id` names a text field.
    pub fn is_field(id: FocusId) -> bool {
        id == PATH
    }

    /// Whether at least one agent can be used.
    fn has_ready_agent(&self) -> bool {
        self.agents.contains(&AgentStatus::Ready)
    }

    /// Adds a project named after `source`, unless it is empty or already added.
    fn add(&mut self, source: &str) {
        let name = name_of(source);
        if !name.is_empty() && !self.projects.iter().any(|project| project == name) {
            self.projects.push(name.to_owned());
        }
    }
}

/// A line of `content` at `pixels` tall type with a line height of `ratio` times that, in `tone`.
fn line(content: impl Into<String>, pixels: u32, ratio: f32, tone: Role) -> Node {
    text(content).styles([text_px(pixels), leading_ratio(ratio), text_color(tone)])
}

/// `name` drawn in `tone` in a square `steps` on a side.
fn glyph(name: IconName, steps: f32, tone: Role) -> Node {
    node().styles([size(steps), text_color(tone)]).icon(name)
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

/// A row of a list of `height` steps: a bordered strip holding its children side by side.
fn row(height: u32) -> Node {
    node().styles([
        flex(),
        items_center(),
        gap(3),
        h(height),
        px(4),
        w_full(),
        rounded(Radius::Xl),
        border(),
        border_color(Role::Border),
    ])
}

/// The bar showing how far through onboarding the reader is.
fn progress(step: Step) -> Node {
    node()
        .styles([flex(), items_center(), gap(3)])
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .children(Step::ALL.map(|candidate| {
                    let tone = if candidate <= step {
                        Role::Text
                    } else {
                        Role::SurfaceActive
                    };
                    node().styles([w(8), h(1.5), rounded_full(), bg(tone)])
                })),
        )
        .child(line(
            format!("Step {} of {}", step as usize + 1, Step::ALL.len()),
            12,
            1.4,
            Role::Subtle,
        ))
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
                            row(18).children([
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
    let can_add = !name_of(state.path.text()).is_empty();
    let added = state.projects.iter().enumerate().map(|(index, name)| {
        row(11).children([
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_center(),
                    size(5),
                    rounded(Radius::Md),
                    bg(AVATARS[index % AVATARS.len()]),
                ])
                .child(
                    text(
                        name.chars()
                            .next()
                            .map(|initial| initial.to_uppercase().to_string())
                            .unwrap_or_default(),
                    )
                    .styles([
                        text_px(11),
                        leading_ratio(1.4),
                        font_semibold(),
                        text_color(Color::White),
                    ]),
                ),
            node()
                .styles([flex_1()])
                .child(line(name.clone(), 13, 1.4, Role::Text)),
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
                .children(AGENTS.iter().zip(&state.agents).enumerate().map(
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
                        row(14).children([
                            glyph(*icon, 5.0, Role::Muted),
                            node().styles([flex(), flex_col(), flex_1()]).children([
                                line(*name, 14, 1.4, Role::Text),
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

/// The buttons under a step: back on the left, the way forward on the right.
fn footer(state: &OnboardingState) -> Node {
    let forward = match state.step {
        Step::Welcome => Button::new("Continue", CONTINUE).size(Size::Lg).build(),
        Step::Project => Button::new("Continue", CONTINUE)
            .size(Size::Lg)
            .disabled(state.projects.is_empty())
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

/// The screen shown once onboarding is over, until the workspace exists.
fn finished() -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            items_center(),
            justify_center(),
            gap(2),
            w_full(),
        ])
        .child(text("Workspace").styles([
            text_px(34),
            leading_ratio(1.15),
            font_semibold(),
            text_color(Role::Text),
        ]))
        .child(line("Nothing here yet.", 15, 1.6, Role::Muted))
}

/// The whole onboarding screen for `state`.
pub fn onboarding(state: &OnboardingState) -> Node {
    let body = if state.finished {
        finished()
    } else {
        let step = match state.step {
            Step::Welcome => welcome(),
            Step::Project => project(state),
            Step::Agents => agents(state),
        };
        node()
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
            .child(footer(state))
    };
    node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            w_full(),
            pm_style::h_full(),
            p(10),
            bg(Role::Background),
        ])
        .child(body)
        .children(state.menu.and_then(|open| menu_over(state, open)))
}

/// The menu for the field it was opened on, if that field still exists.
fn menu_over(state: &OnboardingState, open: ContextMenu) -> Option<Node> {
    let field = state.field(open.field)?;
    let editable = !field.is_read_only();
    Some(
        Menu::new(open.at.0, open.at.1)
            .item(
                "Undo",
                Some("Ctrl+Z"),
                MENU_UNDO,
                !(editable && field.can_undo()),
            )
            .item(
                "Redo",
                Some("Ctrl+Shift+Z"),
                MENU_REDO,
                !(editable && field.can_redo()),
            )
            .separator()
            .item(
                "Cut",
                Some("Ctrl+X"),
                MENU_CUT,
                !(editable && field.can_copy()),
            )
            .item("Copy", Some("Ctrl+C"), MENU_COPY, !field.can_copy())
            .item("Paste", Some("Ctrl+V"), MENU_PASTE, !editable)
            .separator()
            .item(
                "Select all",
                Some("Ctrl+A"),
                MENU_SELECT_ALL,
                field.text().is_empty(),
            )
            .build(),
    )
}

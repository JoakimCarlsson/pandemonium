//! Adding a project: the modal that offers a folder on this computer or a repository to clone, found through the accounts and organizations on GitHub.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, animate_spin, bg, border_b, border_color, cursor_pointer, flex, flex_1, flex_col,
    flex_none, gap, h, items_center, max_h, min_w_px, overflow_hidden, p, px, py, rounded,
    truncate,
};
use pm_text::Field;
use pm_ui::{Node, node};

use crate::Input;
use crate::Modal;
use crate::folder_picker::Purpose;
use crate::input;
use crate::parts::{glyph, line};
use crate::state::{MENU_SCROLL, Outgoing, WorkspaceState};

/// Closes the panel, which is what pressing outside it does.
pub const ADDER_SCRIM: FocusId = FocusId(6000);

/// The panel itself, which keeps a press on it from counting as outside.
pub const ADDER_BODY: FocusId = FocusId(6001);

/// The row offering the desktop's folder chooser.
pub const ADDER_BROWSE: FocusId = FocusId(6003);

/// The row offering a repository to clone.
pub const ADDER_CLONE: FocusId = FocusId(6004);

/// The field the reader types a search or an address into.
pub const ADDER_FIELD: FocusId = FocusId(6006);

/// Takes the row that is highlighted.
pub const ADDER_SUBMIT: FocusId = FocusId(6007);

/// Highlights the row above.
pub const ADDER_UP: FocusId = FocusId(6008);

/// Highlights the row below.
pub const ADDER_DOWN: FocusId = FocusId(6009);

/// Goes back to the first choices, or closes the panel when they are showing.
pub const ADDER_ESCAPE: FocusId = FocusId(6010);

/// The first of the rows of a list; the rest follow it.
pub const ADDER_ROW_BASE: u64 = 6100;

/// How many rows of a list have names.
const ADDER_ROWS: u64 = 500;

/// How wide the panel is, in logical pixels.
const WIDTH: f32 = 632.0;

/// How tall the list of rows is at most, in logical pixels.
const LIST_HEIGHT: f32 = 288.0;

/// What the panel is showing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// The two ways to add a project.
    Choices,
    /// The GitHub accounts and organizations whose repositories can be listed.
    Owners,
    /// The repositories of the account or organization that was picked, or an address typed.
    Repositories,
}

/// What a list of rows holds, or is waiting for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Listing {
    /// The server has been asked and has not answered.
    Loading,
    /// The server could not say, for this reason.
    Failed(String),
    /// What the server said.
    Found(Vec<Found>),
}

/// What a row of a list is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// An account to open, and whether it is an organization.
    Owner(bool),
    /// A repository to clone.
    Repository,
    /// A folder to look inside.
    Folder,
}

/// One thing the server listed: an account to open, a repository to clone or a folder to look inside.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Found {
    /// What it is called.
    pub name: String,
    /// What is said under the name.
    pub detail: String,
    /// The login of an account, the address of a repository or the path of a folder.
    pub value: String,
    /// What it is.
    pub kind: Kind,
}

/// The panel for adding a project, with what the reader typed and what the server listed.
#[derive(Clone, Debug, PartialEq)]
pub struct Adder {
    /// What the panel is showing.
    pub step: Step,
    /// What the reader types.
    pub field: Field,
    /// The accounts and organizations the server listed.
    pub owners: Listing,
    /// The repositories the server listed for the account picked.
    pub repositories: Listing,
    /// The account picked: its login and whether it is an organization.
    pub owner: Option<(String, bool)>,
    /// The row that is highlighted, by position.
    pub selected: usize,
    /// What the server is doing for the reader, while it is.
    pub working: Option<String>,
    /// Why the last thing asked of the server did not work.
    pub trouble: Option<String>,
    /// The server on another machine the project is added on, or this machine's when nothing.
    pub machine: Option<pm_config::KnownServer>,
}

/// What the list says when it has no rows to say it.
pub(crate) enum Status {
    /// Something is being waited for.
    Loading(String),
    /// Something is the matter, or nothing matches.
    Message(String),
}

/// One row as it is drawn.
struct Row {
    /// The glyph at its left.
    icon: IconName,
    /// The name.
    name: String,
    /// What is said under it.
    detail: String,
}

/// Whether `text` is written as the address of a repository rather than as words to search for.
fn is_address(text: &str) -> bool {
    let text = text.trim();
    text.contains("://") || text.starts_with("git@") || text.ends_with(".git")
}

/// Whether every word of `needle` is somewhere in `found`, in any case.
fn matches(found: &Found, needle: &str) -> bool {
    let hay = format!("{} {}", found.name, found.detail).to_lowercase();
    needle
        .to_lowercase()
        .split_whitespace()
        .all(|word| hay.contains(word))
}

impl Adder {
    /// The panel open on its first choices, adding the project on `machine`, or this machine when nothing.
    fn new(machine: Option<pm_config::KnownServer>) -> Self {
        Self {
            step: Step::Choices,
            field: Field::default(),
            owners: Listing::Loading,
            repositories: Listing::Loading,
            owner: None,
            selected: 0,
            working: None,
            trouble: None,
            machine,
        }
    }

    /// The address of the project in the folder `path` of the machine it is added on.
    fn address(&self, path: &str) -> String {
        match &self.machine {
            Some(machine) => machine.project(path),
            None => path.to_owned(),
        }
    }

    /// The panel on `step`, with nothing typed and the first row highlighted.
    fn moved_to(&mut self, step: Step) {
        self.step = step;
        self.field = Field::default();
        self.selected = 0;
        self.trouble = None;
    }

    /// What the server listed for the step showing.
    fn listing(&self) -> &Listing {
        match self.step {
            Step::Repositories => &self.repositories,
            _ => &self.owners,
        }
    }

    /// What the list holds that the reader can take, in order: an address typed, then what the server listed that matches.
    pub(crate) fn entries(&self) -> Vec<Found> {
        if self.step == Step::Choices {
            return Vec::new();
        }
        let typed = self.field.text();
        let mut entries = Vec::new();
        if is_address(typed) {
            entries.push(Found {
                name: typed.trim().to_owned(),
                detail: "Clone from this address".to_owned(),
                value: typed.trim().to_owned(),
                kind: Kind::Repository,
            });
        }
        if let Listing::Found(found) = self.listing() {
            entries.extend(found.iter().filter(|held| matches(held, typed)).cloned());
        }
        entries
    }

    /// How many rows the list has.
    pub(crate) fn count(&self) -> usize {
        match self.step {
            Step::Choices => 2,
            _ => self.entries().len(),
        }
    }

    /// The line saying what the list is waiting for or why it is empty, when it has no rows to say it.
    pub(crate) fn status(&self) -> Option<Status> {
        if let Some(working) = &self.working {
            return Some(Status::Loading(working.clone()));
        }
        if let Some(trouble) = &self.trouble {
            return Some(Status::Message(trouble.clone()));
        }
        if self.step == Step::Choices || self.count() > 0 {
            return None;
        }
        Some(match self.listing() {
            Listing::Loading => Status::Loading(
                match self.step {
                    Step::Owners => "Loading accounts and organizations",
                    _ => "Loading repositories",
                }
                .to_owned(),
            ),
            Listing::Failed(reason) => {
                Status::Message(reason.lines().next().unwrap_or(reason).to_owned())
            }
            Listing::Found(_) => Status::Message("Nothing matches".to_owned()),
        })
    }

    /// What the field says while it is empty.
    fn placeholder(&self) -> &'static str {
        match self.step {
            Step::Owners => "Search accounts and organizations, or paste a Git URL",
            Step::Repositories => "Search repositories, or paste a Git URL",
            Step::Choices => "",
        }
    }

    /// The rows of the first choices, on the machine the project is added on.
    fn choices(&self) -> [(IconName, &'static str, String); 2] {
        let browse = match &self.machine {
            Some(machine) => format!("Choose a folder on {}", machine.name),
            None => "Choose a folder on this computer".to_owned(),
        };
        [
            (IconName::FolderOpen, "Browse", browse),
            (
                IconName::GitFork,
                "Clone from GitHub",
                "Search projects available to your GitHub account".to_owned(),
            ),
        ]
    }

    /// The rows as they are drawn.
    fn rows(&self) -> Vec<Row> {
        match self.step {
            Step::Choices => self
                .choices()
                .into_iter()
                .map(|(icon, name, detail)| Row {
                    icon,
                    name: name.to_owned(),
                    detail,
                })
                .collect(),
            _ => self
                .entries()
                .into_iter()
                .map(|found| Row {
                    icon: match found.kind {
                        Kind::Owner(_) => IconName::CircleUser,
                        Kind::Repository => IconName::GitFork,
                        Kind::Folder => IconName::Folder,
                    },
                    name: found.name,
                    detail: found.detail,
                })
                .collect(),
        }
    }
}

/// The row of the first choices at `index`.
fn choice_id(index: usize) -> FocusId {
    [ADDER_BROWSE, ADDER_CLONE][index.min(1)]
}

/// The modal for adding a project, when it is open.
pub fn adder(state: &WorkspaceState) -> Option<Node> {
    let open = state.adder.as_ref()?;
    Some(
        Modal::new(ADDER_SCRIM, ADDER_BODY)
            .width(WIDTH)
            .children(panel(open, state.menu_scroll))
            .build(),
    )
}

/// What the modal holds: its heading, the field when the step has one and the list.
fn panel(open: &Adder, scrolled: f32) -> Vec<Node> {
    let mut parts = vec![
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                h(11),
                px(4),
                flex_none(),
                border_b(),
                border_color(Role::Border),
            ])
            .child(line("Add project", 13, 1.4, Role::Text))
            .children(heading_hint(open).map(|hint| line(hint, 13, 1.4, Role::Subtle))),
    ];
    if open.step != Step::Choices {
        parts.push(
            node()
                .styles([
                    flex(),
                    items_center(),
                    h(11),
                    px(4),
                    flex_none(),
                    border_b(),
                    border_color(Role::Border),
                ])
                .child(
                    Input::new(&open.field, ADDER_FIELD)
                        .variant(input::Variant::Ghost)
                        .placeholder(open.placeholder())
                        .build(),
                ),
        );
    }
    let mut list = node()
        .styles([
            flex(),
            flex_col(),
            p(1.5),
            max_h(LIST_HEIGHT / 4.0),
            overflow_hidden(),
        ])
        .scroll_from_start(MENU_SCROLL, scrolled);
    list = list.children(open.rows().iter().enumerate().map(|(index, row)| {
        let id = if open.step == Step::Choices {
            choice_id(index)
        } else {
            FocusId(ADDER_ROW_BASE + index as u64)
        };
        entry(row, id, index == open.selected)
    }));
    if let Some(status) = open.status() {
        list = list.child(status_line(status));
    }
    parts.push(list);
    parts
}

/// What is said after the heading: whose repositories are listed, once an account is picked.
fn heading_hint(open: &Adder) -> Option<String> {
    match (&open.owner, open.step) {
        (Some((login, _)), Step::Repositories) => Some(login.clone()),
        _ => None,
    }
}

/// The line under the rows: a spinner and what is being waited for, or what is the matter.
pub(crate) fn status_line(status: Status) -> Node {
    let row = node().styles([flex(), items_center(), gap(3), px(2.5), py(2)]);
    match status {
        Status::Loading(message) => row
            .child(glyph(IconName::Loading, 4.0, Role::Muted).styles([animate_spin()]))
            .child(line(message, 13, 1.4, Role::Subtle)),
        Status::Message(message) => row.child(line(message, 13, 1.4, Role::Subtle)),
    }
}

/// One row of the list: a glyph, a name and what is said under it, highlighted while it is the one the keys act on.
fn entry(row: &Row, id: FocusId, selected: bool) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        gap(3),
        px(2.5),
        py(1.5),
        flex_none(),
        rounded(Radius::Lg),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if selected {
        styles.push(bg(Role::SurfaceSelected));
    }
    node()
        .styles(styles)
        .child(glyph(row.icon, 4.0, Role::Muted))
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_w_px(0)])
                .child(line(row.name.clone(), 13, 1.4, Role::Text).styles([truncate()]))
                .children(
                    (!row.detail.is_empty()).then(|| {
                        line(row.detail.clone(), 12, 1.4, Role::Subtle).styles([truncate()])
                    }),
                ),
        )
        .focusable(id)
}

/// Whether `id` names something of the panel.
pub fn is_adder(id: FocusId) -> bool {
    let FocusId(number) = id;
    (ADDER_SCRIM.0..=ADDER_ESCAPE.0).contains(&number)
        || (ADDER_ROW_BASE..ADDER_ROW_BASE + ADDER_ROWS).contains(&number)
}

impl WorkspaceState {
    /// Whether the panel for adding a project is open.
    pub fn adder_open(&self) -> bool {
        self.adder.is_some()
    }

    /// What pressing Enter presses while the panel is open: the highlighted choice, or the highlighted row of a list.
    pub fn adder_enter(&self) -> Option<FocusId> {
        let open = self.adder.as_ref()?;
        Some(match open.step {
            Step::Choices => choice_id(open.selected),
            _ => ADDER_SUBMIT,
        })
    }

    /// The state with the panel closed.
    pub fn without_adder(mut self) -> Self {
        self.adder = None;
        self.menu_scroll = 0.0;
        self
    }

    /// The state after something of the panel was pressed: `action` is the name of what.
    pub(crate) fn adder_pressed(mut self, action: FocusId) -> Self {
        let Some(mut open) = self.adder.take() else {
            return self;
        };
        let working = open.working.is_some();
        match action {
            ADDER_SCRIM => return self.without_adder(),
            ADDER_ESCAPE if open.step == Step::Choices => return self.without_adder(),
            ADDER_ESCAPE => {
                open.moved_to(match open.step {
                    Step::Repositories => Step::Owners,
                    _ => Step::Choices,
                });
                self.menu_scroll = 0.0;
            }
            ADDER_UP | ADDER_DOWN if !working => {
                let last = open.count().saturating_sub(1);
                open.selected = match (open.selected, action) {
                    (usize::MAX, _) => 0,
                    (selected, ADDER_UP) => selected.saturating_sub(1),
                    (selected, _) => (selected + 1).min(last),
                };
                self.keep_selected_in_view(open.selected);
            }
            ADDER_SUBMIT if !working => {
                let index = if open.selected == usize::MAX {
                    0
                } else {
                    open.selected
                };
                self.adder = Some(open);
                self.adder_chosen(index);
                return self;
            }
            ADDER_BROWSE if !working => {
                self.menu_scroll = 0.0;
                self.open_folder_picker(Purpose::Open, open.machine);
                return self;
            }
            ADDER_CLONE if !working => {
                open.moved_to(Step::Owners);
                open.owners = Listing::Loading;
                self.menu_scroll = 0.0;
                self.outbox.push(Outgoing::Owners);
            }
            _ => {}
        }
        self.adder = Some(open);
        self
    }

    /// Takes the row of a list at `index`: opens the account it names, or asks the server to clone the repository it names.
    pub(crate) fn adder_chosen(&mut self, index: usize) {
        let Some(open) = self.adder.as_mut() else {
            return;
        };
        let entries = open.entries();
        let Some(found) = entries.get(index) else {
            return;
        };
        match found.kind {
            Kind::Owner(organization) => {
                let login = found.value.clone();
                open.moved_to(Step::Repositories);
                open.repositories = Listing::Loading;
                open.owner = Some((login.clone(), organization));
                self.menu_scroll = 0.0;
                self.outbox.push(Outgoing::Repositories {
                    login,
                    organization,
                });
            }
            Kind::Repository => {
                let (repository, machine) = (found.clone(), open.machine.clone());
                self.open_folder_picker(Purpose::CloneInto(repository), machine);
            }
            Kind::Folder => {}
        }
    }

    /// The state with the panel open on its first choices.
    pub(crate) fn with_adder(mut self) -> Self {
        self.menu = None;
        self.picker = None;
        self.menu_scroll = 0.0;
        self.adder = Some(Adder::new(self.window.servers.default_target().cloned()));
        self
    }

    /// The state after `edit` was made to the field of the panel.
    pub(crate) fn adder_edited(&mut self, edit: pm_text::Edit) {
        let Some(open) = self.adder.as_mut() else {
            return;
        };
        let before = open.field.text().to_owned();
        open.field.apply(edit);
        if open.field.text() != before {
            open.selected = 0;
            open.trouble = None;
            self.menu_scroll = 0.0;
        }
    }

    /// The state with the accounts and organizations the server listed, or why it could not.
    pub fn with_owners(mut self, listed: Result<Vec<Found>, String>) -> Self {
        if let Some(open) = self.adder.as_mut() {
            open.owners = Self::listing_of(listed);
        }
        self
    }

    /// The state with the repositories the server listed for `login`, or why it could not, when that is still the account showing.
    pub fn with_repositories(mut self, login: &str, listed: Result<Vec<Found>, String>) -> Self {
        if let Some(open) = self.adder.as_mut()
            && open.owner.as_ref().is_some_and(|(held, _)| held == login)
        {
            open.repositories = Self::listing_of(listed);
        }
        self
    }

    /// What a list holds, from what the server answered.
    fn listing_of(listed: Result<Vec<Found>, String>) -> Listing {
        match listed {
            Ok(found) => Listing::Found(found),
            Err(reason) => Listing::Failed(reason),
        }
    }

    /// The state after the server cloned the repository into `path`, which is added as a project and opened.
    pub fn adder_landed(self, path: &str) -> Self {
        let address = self
            .adder
            .as_ref()
            .map_or_else(|| path.to_owned(), |open| open.address(path));
        self.without_adder().with_project_opened(&address)
    }

    /// The state after the server could not do what the panel asked, for this reason.
    pub fn adder_failed(mut self, reason: &str) -> Self {
        if let Some(open) = self.adder.as_mut() {
            open.working = None;
            open.trouble = Some(reason.lines().next().unwrap_or(reason).to_owned());
        }
        self
    }

    /// Scrolls the list so that the row at `selected` is in view.
    fn keep_selected_in_view(&mut self, selected: usize) {
        let row = ENTRY_HEIGHT;
        let top = selected as f32 * row;
        let bottom = top + row;
        if top < self.menu_scroll {
            self.menu_scroll = top;
        } else if bottom > self.menu_scroll + LIST_HEIGHT - 12.0 {
            self.menu_scroll = bottom - LIST_HEIGHT + 12.0;
        }
    }
}

/// How tall a row of the list is, in logical pixels.
const ENTRY_HEIGHT: f32 = 48.0;

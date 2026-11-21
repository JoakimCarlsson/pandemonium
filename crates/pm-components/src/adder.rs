//! Adding a project: the modal that offers a folder on this computer or a repository to clone, found through the accounts and organizations on GitHub.

use crate::areas::DialogEvent;
use crate::events;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::{Alignment, Arrangement};
use rustle::{Icon, phosphor};

use crate::Input;
use crate::Modal;
use crate::folder_picker::Purpose;
use crate::icons::spinner;
use crate::input;
use crate::parts::{glyph, line};
use crate::theme::{Radius, Role, sp};

use crate::state::{Outgoing, WorkspaceState};

/// How many rows of a list have names.
const ADDER_ROWS: usize = 500;

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
    pub field: TextFieldState,
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
    /// Where the list of rows is scrolled.
    pub list: rustle::LazyListState,
}

/// What the list says when it has no rows to say it.
pub(crate) enum Status {
    /// Something is being waited for.
    Loading(String),
    /// Something is the matter, or nothing matches.
    Message(String),
}

/// One row as it is drawn: the glyph at its left, the name and what is said under it.
type Row = (Icon, String, String);

impl Listing {
    /// What a list showing none of its rows says: that `loading` is waited for, why the server could not say, that `empty` when it found nothing, or that nothing matches what was typed.
    pub(crate) fn standing(&self, loading: &str, empty: &str) -> Status {
        match self {
            Listing::Loading => Status::Loading(loading.to_owned()),
            Listing::Failed(reason) => {
                Status::Message(reason.lines().next().unwrap_or(reason).to_owned())
            }
            Listing::Found(inside) if inside.is_empty() => Status::Message(empty.to_owned()),
            Listing::Found(_) => Status::Message("Nothing matches".to_owned()),
        }
    }
}

/// Whether `text` is written as the address of a repository rather than as words to search for.
fn is_address(text: &str) -> bool {
    let text = text.trim();
    text.contains("://") || text.starts_with("git@") || text.ends_with(".git")
}

/// Whether every word of `needle` is somewhere in `found`, in any case.
pub(crate) fn matches(found: &Found, needle: &str) -> bool {
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
            field: TextFieldState::default(),
            owners: Listing::Loading,
            repositories: Listing::Loading,
            owner: None,
            selected: 0,
            working: None,
            trouble: None,
            machine,
            list: rustle::LazyListState::default(),
        }
    }

    /// The panel on `step`, with nothing typed and the first row highlighted.
    fn moved_to(&mut self, step: Step) {
        self.step = step;
        self.field = TextFieldState::default();
        self.selected = 0;
        self.trouble = None;
        self.list.scroll().jump_to(0.0);
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
        let typed = &self.field.text();
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
        let loading = match self.step {
            Step::Owners => "Loading accounts and organizations",
            _ => "Loading repositories",
        };
        Some(self.listing().standing(loading, "Nothing matches"))
    }

    /// What the field says while it is empty.
    fn placeholder(&self) -> &'static str {
        match self.step {
            Step::Owners => "Search accounts and organizations, or paste a Git URL",
            Step::Repositories => "Search repositories, or paste a Git URL",
            Step::Choices => "",
        }
    }

    /// The rows as they are drawn: the first choices, on the machine the project is added on, or the entries of a list.
    fn rows(&self) -> Vec<Row> {
        if self.step == Step::Choices {
            let browse = match &self.machine {
                Some(machine) => format!("Choose a folder on {}", machine.name),
                None => "Choose a folder on this computer".to_owned(),
            };
            let clone = "Search projects available to your GitHub account".to_owned();
            return vec![
                (phosphor::FOLDER_OPEN, "Browse".to_owned(), browse),
                (phosphor::GIT_FORK, "Clone from GitHub".to_owned(), clone),
            ];
        }
        self.entries()
            .into_iter()
            .map(|found| {
                let icon = match found.kind {
                    Kind::Owner(_) => phosphor::USER_CIRCLE,
                    Kind::Repository => phosphor::GIT_FORK,
                    Kind::Folder => phosphor::FOLDER,
                };
                (icon, found.name, found.detail)
            })
            .collect()
    }
}

/// The row of the first choices at `index`.
fn choice_id(index: usize) -> Target {
    [
        Target::Dialog(DialogEvent::AdderBrowse),
        Target::Dialog(DialogEvent::AdderClone),
    ][index.min(1)]
}

/// Emits the modal for adding a project, when it is open.
#[track_caller]
pub fn adder(state: &WorkspaceState) {
    let Some(open) = state.adder.as_ref() else {
        return;
    };
    Modal::new(
        Target::Dialog(DialogEvent::AdderScrim),
        Target::Dialog(DialogEvent::AdderBody),
    )
    .width(WIDTH)
    .build_with(|| panel(open));
}

/// Emits what the modal holds: its heading, the field when the step has one and the list, scrolled as the panel's list state says.
#[track_caller]
fn panel(open: &Adder) {
    let banded = || {
        Modifier::new()
            .height(theme::sp(11))
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(4), 0.0)
    };
    rustle::row_arranged(
        banded(),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            line("Add project", 13, 1.4, Role::Text);
            if let (Some((login, _)), Step::Repositories) = (&open.owner, open.step) {
                line(login.clone(), 13, 1.4, Role::Subtle);
            }
        },
    );
    if open.step != Step::Choices {
        rustle::row_arranged(banded(), Arrangement::Start, Alignment::Center, || {
            Input::new(&open.field, Target::Dialog(DialogEvent::AdderField))
                .variant(input::Variant::Ghost)
                .placeholder(open.placeholder())
                .modifier(Modifier::new().weight(1.0))
                .build();
        });
    }
    let rows = open.rows();
    let status = open.status();
    let count = rows.len() + usize::from(status.is_some());
    rustle::lazy_list_with(
        Modifier::new()
            .height_in(0.0, sp(LIST_HEIGHT / 4.0))
            .fill_max_width(),
        &open.list,
        rustle::LazyContent::new().padding(theme::sp(1.5)),
        count,
        |index| index,
        |index| {
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .padding_xy(theme::sp(1.5), 0.0),
                Arrangement::Start,
                Alignment::Start,
                || match rows.get(index) {
                    Some(row) => {
                        let id = if open.step == Step::Choices {
                            choice_id(index)
                        } else {
                            Target::Dialog(DialogEvent::AdderRow(index))
                        };
                        entry(row, id, index == open.selected);
                    }
                    None => {
                        if let Some(status) = open.status() {
                            status_line(status);
                        }
                    }
                },
            );
        },
    );
}

/// Emits the line under the rows: a spinner and what is being waited for, or what is the matter.
#[track_caller]
pub(crate) fn status_line(status: Status) {
    rustle::row_arranged(
        Modifier::new().padding_xy(theme::sp(2.5), theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Center,
        || match status {
            Status::Loading(message) => {
                spinner(4.0, Role::Muted);
                line(message, 13, 1.4, Role::Subtle);
            }
            Status::Message(message) => line(message, 13, 1.4, Role::Subtle),
        },
    );
}

/// Emits one row of the list: a glyph, a name and what is said under it, highlighted while it is the one the keys act on.
#[track_caller]
fn entry((icon, name, detail): &Row, id: Target, selected: bool) {
    let looks = theme::InteractionTones::new()
        .hover(Role::SurfaceHover)
        .radius(Radius::Lg);
    let looks = if selected {
        looks.idle(Role::SurfaceSelected)
    } else {
        looks
    };
    let looks = Modifier::new()
        .fill_max_width()
        .indicated(looks, theme::Focused::Control(crate::focus::is_focused(id)));
    rustle::row_arranged(
        events::pressed(looks, id).padding_xy(theme::sp(2.5), theme::sp(1.5)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Center,
        || {
            glyph(icon.clone(), 4.0, Role::Muted);
            rustle::column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    line(name.clone(), 13, 1.4, Role::Text);
                    if !detail.is_empty() {
                        line(detail.clone(), 12, 1.4, Role::Subtle);
                    }
                },
            );
        },
    );
}

/// Whether `id` names something of the panel.
pub fn is_adder(id: Target) -> bool {
    matches!(
        id,
        Target::Dialog(DialogEvent::AdderScrim)
            | Target::Dialog(DialogEvent::AdderBody)
            | Target::Dialog(DialogEvent::AdderBrowse)
            | Target::Dialog(DialogEvent::AdderClone)
            | Target::Dialog(DialogEvent::AdderField)
            | Target::Dialog(DialogEvent::AdderSubmit)
            | Target::Dialog(DialogEvent::AdderUp)
            | Target::Dialog(DialogEvent::AdderDown)
            | Target::Dialog(DialogEvent::AdderEscape)
    ) || matches!(id, Target::Dialog(DialogEvent::AdderRow(row)) if row < ADDER_ROWS)
}

impl WorkspaceState {
    /// Whether the panel for adding a project is open.
    pub fn adder_open(&self) -> bool {
        self.adder.is_some()
    }

    /// What pressing Enter presses while the panel is open: the highlighted choice, or the highlighted row of a list.
    pub fn adder_enter(&self) -> Option<Target> {
        let open = self.adder.as_ref()?;
        Some(match open.step {
            Step::Choices => choice_id(open.selected),
            _ => Target::Dialog(DialogEvent::AdderSubmit),
        })
    }

    /// The state with the panel closed.
    pub fn without_adder(mut self) -> Self {
        self.adder = None;
        self.menu_scroll.jump_to(0.0);
        self
    }

    /// The state after something of the panel was pressed: `action` is the name of what.
    pub(crate) fn adder_pressed(mut self, action: Target) -> Self {
        let Some(mut open) = self.adder.take() else {
            return self;
        };
        let working = open.working.is_some();
        match action {
            Target::Dialog(DialogEvent::AdderScrim) => return self.without_adder(),
            Target::Dialog(DialogEvent::AdderEscape) if open.step == Step::Choices => {
                return self.without_adder();
            }
            Target::Dialog(DialogEvent::AdderEscape) => {
                open.moved_to(match open.step {
                    Step::Repositories => Step::Owners,
                    _ => Step::Choices,
                });
            }
            Target::Dialog(DialogEvent::AdderUp | DialogEvent::AdderDown) if !working => {
                let up = action == Target::Dialog(DialogEvent::AdderUp);
                open.selected = stepped(open.selected, up, open.count());
                crate::rows_in_view::keep_in_view(&open.list, open.selected);
            }
            Target::Dialog(DialogEvent::AdderSubmit) if !working => {
                let index = Some(open.selected)
                    .filter(|held| *held != usize::MAX)
                    .unwrap_or(0);
                self.adder = Some(open);
                self.adder_chosen(index);
                return self;
            }
            Target::Dialog(DialogEvent::AdderBrowse) if !working => {
                self.menu_scroll.jump_to(0.0);
                self.open_folder_picker(Purpose::Open, open.machine);
                return self;
            }
            Target::Dialog(DialogEvent::AdderClone) if !working => {
                open.moved_to(Step::Owners);
                open.owners = Listing::Loading;
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
        self.menu_scroll.jump_to(0.0);
        self.adder = Some(Adder::new(self.window.servers.default_target().cloned()));
        self
    }

    /// Takes in what the reader wrote in the field of the panel: the list starts over from its first row.
    pub(crate) fn adder_typed(&mut self) {
        let Some(open) = self.adder.as_mut() else {
            return;
        };
        open.selected = 0;
        open.trouble = None;
        open.list.scroll().jump_to(0.0);
    }

    /// The state with the accounts and organizations the server listed, or why it could not.
    pub fn with_owners(mut self, listed: Result<Vec<Found>, String>) -> Self {
        if let Some(open) = self.adder.as_mut() {
            open.owners = listed.map_or_else(Listing::Failed, Listing::Found);
        }
        self
    }

    /// The state with the repositories the server listed for `login`, or why it could not, when that is still the account showing.
    pub fn with_repositories(mut self, login: &str, listed: Result<Vec<Found>, String>) -> Self {
        if let Some(open) = self.adder.as_mut()
            && open.owner.as_ref().is_some_and(|(held, _)| held == login)
        {
            open.repositories = listed.map_or_else(Listing::Failed, Listing::Found);
        }
        self
    }

    /// The state after the server cloned the repository into `path`, which is added as a project and opened.
    pub fn adder_landed(self, path: &str) -> Self {
        let address = self
            .adder
            .as_ref()
            .and_then(|open| open.machine.as_ref())
            .map_or_else(|| path.to_owned(), |machine| machine.project(path));
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
}

/// The row of a list of `count` rows highlighted after moving `up`, or down, from `selected`: the first while none was.
pub(crate) fn stepped(selected: usize, up: bool, count: usize) -> usize {
    match selected {
        usize::MAX => 0,
        held if up => held.saturating_sub(1),
        held => (held + 1).min(count.saturating_sub(1)),
    }
}

//! The folder picker: a window of its own for choosing a folder on the machine a server runs on.
//! Places on its left, the folders of the one being looked inside as a list, the path above as a
//! trail with the steps back, forward and up, and the button that takes the folder being looked
//! inside. It is what every part of the app that needs a folder opens, for this machine and for a
//! remote one alike.

use crate::areas::DialogEvent;
use crate::parts;
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, TextOverflow};
use rustle::{Icon, phosphor};

use std::sync::atomic::{AtomicU64, Ordering};

use pm_config::KnownServer;
use rustle::Modifier;
use rustle::TextFieldState;

use crate::adder::{Found, Kind, Listing, Status, matches, status_line, stepped};
use crate::button::Variant;
use crate::icons::icon;
use crate::parts::{line, spacer};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::theme::Role;
use crate::tile::Tile;
use crate::{Button, Input, popover};

/// How wide the column of places is, in logical pixels.
const PLACES_WIDTH: f32 = 176.0;

/// How wide the box filtering the folders is, in logical pixels.
const FILTER_WIDTH: f32 = 220.0;

/// How many times a picker has been opened, which numbers the openings.
static OPENINGS: AtomicU64 = AtomicU64::new(0);

/// The places that are one press away: what they are called, where they are and their glyph.
pub static PLACES: [(&str, &str, Icon); 5] = [
    ("Home", "~", phosphor::FOLDER),
    ("Desktop", "~/Desktop", phosphor::FOLDER),
    ("Documents", "~/Documents", phosphor::FOLDER),
    ("Downloads", "~/Downloads", phosphor::FOLDER),
    ("Root", "/", phosphor::FOLDER),
];

/// What the folder chosen is for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Purpose {
    /// A folder to open as a project.
    Open,
    /// A folder to clone this repository into.
    CloneInto(Found),
    /// A folder holding an extension to install.
    Extension,
}

/// What pressing something of the picker came to.
#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Outcome {
    /// The picker stays open, and the server is asked for a folder's contents when there is something to ask.
    Staying(Option<Outgoing>),
    /// The picker was put away without a choice.
    Closed,
    /// The folder at this path was chosen.
    Chosen(String),
}

/// The folder picker, with the folders the server listed and the folders looked inside before.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderPicker {
    /// Which opening of the picker this is, so that a window already showing one can tell a picker opened again.
    pub serial: u64,
    /// What the folder chosen is for.
    pub purpose: Purpose,
    /// The server on another machine whose folders are shown, or this machine's when nothing.
    pub machine: Option<KnownServer>,
    /// What the folders are filtered with.
    pub field: TextFieldState,
    /// The folders the server listed for the folder being looked inside.
    pub folders: Listing,
    /// The folder being looked inside, as the server gave it.
    pub directory: Option<String>,
    /// The folder the server was last asked about, to tell whether an answer is the one wanted.
    pub requested: String,
    /// The folder that is highlighted, by position; nothing while the list was just entered.
    pub selected: usize,
    /// Where the list of folders is scrolled.
    pub list: rustle::LazyListState,
    /// The folders looked inside before the one showing, the latest last.
    behind: Vec<String>,
    /// The folders that going back left, the latest last.
    ahead: Vec<String>,
}

/// The folders that lead to `path`, nearest the root first: what each is called and where it is.
pub fn trail(path: &str) -> Vec<(String, String)> {
    let mut crumbs = vec![("/".to_owned(), "/".to_owned())];
    let mut at = String::new();
    for name in path.split('/').filter(|name| !name.is_empty()) {
        at = format!("{at}/{name}");
        crumbs.push((name.to_owned(), at.clone()));
    }
    crumbs
}

/// Whether `id` names something of the folder picker.
pub fn is_picker(id: Target) -> bool {
    matches!(
        id,
        Target::Dialog(DialogEvent::PickerClose)
            | Target::Dialog(DialogEvent::PickerConfirm)
            | Target::Dialog(DialogEvent::PickerField)
            | Target::Dialog(DialogEvent::PickerSubmit)
            | Target::Dialog(DialogEvent::PickerUp)
            | Target::Dialog(DialogEvent::PickerDown)
            | Target::Dialog(DialogEvent::PickerBack)
            | Target::Dialog(DialogEvent::PickerForward)
            | Target::Dialog(DialogEvent::PickerParent)
            | Target::Dialog(DialogEvent::PickerCrumb(_))
            | Target::Dialog(DialogEvent::PickerPlace(_))
            | Target::Dialog(DialogEvent::PickerRow(_))
    )
}

impl FolderPicker {
    /// The picker for `purpose` looking inside the home folder of `machine`, or of this machine when nothing, with the request that lists it.
    pub fn opened(purpose: Purpose, machine: Option<KnownServer>) -> (Self, Outgoing) {
        let mut picker = Self {
            serial: OPENINGS.fetch_add(1, Ordering::Relaxed),
            purpose,
            machine,
            field: TextFieldState::default(),
            folders: Listing::Loading,
            directory: None,
            requested: String::new(),
            selected: usize::MAX,
            list: rustle::LazyListState::default(),
            behind: Vec::new(),
            ahead: Vec::new(),
        };
        let ask = picker.visit("~");
        (picker, ask)
    }

    /// What the window is called.
    pub fn title(&self) -> String {
        match &self.purpose {
            Purpose::Open => "Open project folder".to_owned(),
            Purpose::CloneInto(repository) => format!("Clone {} into", repository.name),
            Purpose::Extension => "Install extension from folder".to_owned(),
        }
    }

    /// What the button taking the folder says.
    fn confirm_label(&self) -> &'static str {
        match self.purpose {
            Purpose::Open => "Open",
            Purpose::CloneInto(_) => "Clone here",
            Purpose::Extension => "Install",
        }
    }

    /// The machine whose folders are shown, as a request names it, or nothing for this one.
    pub fn host(&self) -> Option<String> {
        self.machine.as_ref().map(KnownServer::host)
    }

    /// The address of the project in the folder `path` of the machine shown.
    pub fn address(&self, path: &str) -> String {
        match &self.machine {
            Some(machine) => machine.project(path),
            None => path.to_owned(),
        }
    }

    /// The picker looking inside `path` without recording where it was, with the server asked about it.
    fn visit(&mut self, path: &str) -> Outgoing {
        self.folders = Listing::Loading;
        self.requested = path.to_owned();
        self.field = TextFieldState::default();
        self.selected = usize::MAX;
        self.list.scroll().jump_to(0.0);
        Outgoing::Folders {
            path: path.to_owned(),
            host: self.host(),
        }
    }

    /// The picker looking inside `path`, remembering the folder left so that going back returns to it.
    fn go(&mut self, path: &str) -> Outgoing {
        if let Some(left) = self.directory.clone() {
            self.behind.push(left);
        }
        self.ahead.clear();
        self.visit(path)
    }

    /// The picker looking inside the folder it was in before, going `back`, or the one that going back left otherwise, when there is one.
    fn travel(&mut self, back: bool) -> Option<Outgoing> {
        let (from, to) = if back {
            (&mut self.behind, &mut self.ahead)
        } else {
            (&mut self.ahead, &mut self.behind)
        };
        let path = from.pop()?;
        to.extend(self.directory.clone());
        Some(self.visit(&path))
    }

    /// The picker looking inside the folder that holds the one showing, when it is not the root.
    fn go_up(&mut self) -> Option<Outgoing> {
        let crumbs = trail(self.directory.as_deref()?);
        let (_, parent) = crumbs.get(crumbs.len().checked_sub(2)?)?;
        let parent = parent.clone();
        Some(self.go(&parent))
    }

    /// The folders inside the one being looked into that match what was typed.
    pub fn entries(&self) -> Vec<Found> {
        let Listing::Found(inside) = &self.folders else {
            return Vec::new();
        };
        let typed = &self.field.text();
        inside
            .iter()
            .filter(|held| matches(held, typed))
            .cloned()
            .collect()
    }

    /// The line saying what the list is waiting for or why it is empty, when it has no folders to say it.
    fn status(&self) -> Option<Status> {
        if !self.entries().is_empty() {
            return None;
        }
        Some(self.folders.standing("Loading folders", "No folders here"))
    }

    /// What pressing Enter presses: the folder being looked inside when nothing is typed, and the highlighted folder otherwise.
    pub fn enter(&self) -> Target {
        if self.field.text().trim().is_empty() {
            Target::Dialog(DialogEvent::PickerConfirm)
        } else {
            Target::Dialog(DialogEvent::PickerSubmit)
        }
    }

    /// Looks inside the folder of the list at `index`.
    fn open_row(&mut self, index: usize) -> Option<Outgoing> {
        let found = self.entries().get(index).cloned()?;
        Some(self.go(&found.value))
    }

    /// What pressing `id` does to the picker.
    pub fn pressed(&mut self, id: Target) -> Outcome {
        match id {
            Target::Dialog(DialogEvent::PickerClose) => Outcome::Closed,
            Target::Dialog(DialogEvent::PickerConfirm) => match self.directory.clone() {
                Some(path) => Outcome::Chosen(path),
                None => Outcome::Staying(None),
            },
            Target::Dialog(DialogEvent::PickerSubmit) => {
                let index = Some(self.selected)
                    .filter(|held| *held != usize::MAX)
                    .unwrap_or(0);
                Outcome::Staying(self.open_row(index))
            }
            Target::Dialog(DialogEvent::PickerUp | DialogEvent::PickerDown) => {
                let up = id == Target::Dialog(DialogEvent::PickerUp);
                self.selected = stepped(self.selected, up, self.entries().len());
                crate::rows_in_view::keep_in_view(&self.list, self.selected);
                Outcome::Staying(None)
            }
            Target::Dialog(DialogEvent::PickerBack) => Outcome::Staying(self.travel(true)),
            Target::Dialog(DialogEvent::PickerForward) => Outcome::Staying(self.travel(false)),
            Target::Dialog(DialogEvent::PickerParent) => Outcome::Staying(self.go_up()),
            part => Outcome::Staying(self.pressed_part(part)),
        }
    }

    /// What pressing the part of the path, the place or the folder `part` does.
    fn pressed_part(&mut self, part: Target) -> Option<Outgoing> {
        match part {
            Target::Dialog(DialogEvent::PickerCrumb(index)) => {
                let crumbs = trail(self.directory.as_deref()?);
                let (_, path) = crumbs.get(index)?.clone();
                Some(self.go(&path))
            }
            Target::Dialog(DialogEvent::PickerPlace(index)) => {
                let (_, path, _) = PLACES.get(index)?;
                Some(self.go(path))
            }
            Target::Dialog(DialogEvent::PickerRow(index)) => self.open_row(index),
            _ => None,
        }
    }

    /// The picker after the reader changed what is typed: the first folder is selected and the list goes back to its top.
    pub fn typed(&mut self) {
        self.selected = 0;
        self.list.scroll().jump_to(0.0);
    }

    /// The picker with the folders the server listed for `requested`, or why it could not, when that is still the folder wanted.
    pub fn listed(&mut self, requested: &str, listed: Result<(String, Vec<String>), String>) {
        if self.requested != requested {
            return;
        }
        match listed {
            Ok((path, names)) => {
                self.folders = Listing::Found(
                    names
                        .into_iter()
                        .map(|name| Found {
                            value: match path.as_str() {
                                "/" => format!("/{name}"),
                                _ => format!("{path}/{name}"),
                            },
                            name,
                            detail: String::new(),
                            kind: Kind::Folder,
                        })
                        .collect(),
                );
                self.directory = Some(path);
            }
            Err(reason) => self.folders = Listing::Failed(reason),
        }
    }

    /// Emits what the window holds: the bar with the steps and the path, the places beside the folders, and the bar with the buttons.
    #[track_caller]
    pub fn view(&self) {
        rustle::column_arranged(
            Modifier::new()
                .weight(1.0)
                .fill_max_width()
                .chain(crate::focus::keeps_keyboard_in(Target::Dialog(
                    DialogEvent::PickerField,
                ))),
            Arrangement::Start,
            Alignment::Start,
            || {
                self.heading();
                self.bar();
                rustle::row_arranged(
                    Modifier::new().fill_max_width().weight(1.0).clip(),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        places();
                        self.list();
                    },
                );
                self.footer();
            },
        );
    }

    /// Emits the line naming what the folder is for, and the machine it is on when that is not this one.
    #[track_caller]
    fn heading(&self) {
        parts::bar(11, 4, 2, || {
            line(self.title(), 13, 1.4, Role::Text);
            if let Some(machine) = self.machine.as_ref() {
                line(format!("on {}", machine.name), 13, 1.4, Role::Subtle);
            }
        });
    }

    /// Emits the bar above the folders: the steps back, forward and up, the path as a trail that can be pressed anywhere along, and the box filtering the folders.
    #[track_caller]
    fn bar(&self) {
        let crumbs = self.directory.as_deref().map(trail).unwrap_or_default();
        let last = crumbs.len().saturating_sub(1);
        parts::bar(12, 3, 2, || {
            step(
                phosphor::ARROW_LEFT,
                DialogEvent::PickerBack,
                self.behind.is_empty(),
                "Back",
            );
            step(
                phosphor::ARROW_RIGHT,
                DialogEvent::PickerForward,
                self.ahead.is_empty(),
                "Forward",
            );
            step(
                phosphor::ARROW_UP,
                DialogEvent::PickerParent,
                crumbs.len() < 2,
                "Up",
            );
            rustle::row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::SpacedBy(theme::sp(0.5)),
                Alignment::Center,
                || {
                    for (index, (name, _)) in crumbs.iter().enumerate() {
                        if index > 0 {
                            icon(phosphor::CARET_RIGHT, 3.0, Role::Subtle);
                        }
                        let kind = if index == last {
                            theme::strong(13, Role::Text)
                        } else {
                            theme::line(13, Role::Muted)
                        };
                        Tile::new(Target::Dialog(DialogEvent::PickerCrumb(index)))
                            .height(7.0)
                            .inner(Modifier::new().padding_xy(theme::sp(1.5), 0.0))
                            .build_with(|| {
                                rustle::basic_text(
                                    Modifier::new(),
                                    name.clone(),
                                    kind,
                                    TextOverflow::Ellipsis,
                                )
                            });
                    }
                },
            );
            Input::new(&self.field, Target::Dialog(DialogEvent::PickerField))
                .size(Size::Sm)
                .placeholder("Filter folders")
                .modifier(Modifier::new().width(FILTER_WIDTH))
                .submits(Target::Dialog(DialogEvent::PickerSubmit))
                .build();
        });
    }

    /// Emits the folders inside the one being looked into, one to a line, scrolled to where the picker says.
    #[track_caller]
    fn list(&self) {
        let entries = self.entries();
        let count = entries.len() + usize::from(self.status().is_some());
        rustle::lazy_list_with(
            Modifier::new()
                .fill_max_height()
                .weight(1.0)
                .fill_max_width(),
            &self.list,
            rustle::LazyContent::new()
                .padding(theme::sp(2))
                .spacing(theme::sp(0.5)),
            count,
            |index| index,
            |index| {
                rustle::column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || match entries.get(index) {
                        Some(found) => folder_row(&found.name, index == self.selected, index),
                        None => {
                            if let Some(status) = self.status() {
                                status_line(status);
                            }
                        }
                    },
                );
            },
        );
    }

    /// Emits the bar under the folders: the key that closes the picker, and the buttons that close it and that take the folder.
    #[track_caller]
    fn footer(&self) {
        rustle::column_arranged(
            Modifier::new().fill_max_width().height(theme::sp(14)),
            Arrangement::Start,
            Alignment::Start,
            || {
                rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
                rustle::row_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .weight(1.0)
                        .padding_xy(theme::sp(4), 0.0),
                    Arrangement::SpacedBy(theme::sp(2)),
                    Alignment::Center,
                    || {
                        popover::key_hint("Esc", "Close");
                        spacer();
                        Button::new("Cancel", Target::Dialog(DialogEvent::PickerClose))
                            .variant(Variant::Ghost)
                            .build();
                        Button::new(
                            self.confirm_label(),
                            Target::Dialog(DialogEvent::PickerConfirm),
                        )
                        .disabled(self.directory.is_none())
                        .build();
                    },
                );
            },
        );
    }
}

/// Emits the icon button of the step `event` in the bar, showing `glyph` and saying `tip` on hover, greyed out while `disabled`.
#[track_caller]
fn step(glyph: Icon, event: DialogEvent, disabled: bool, tip: &str) {
    Button::icon(glyph, Target::Dialog(event))
        .disabled(disabled)
        .tooltip(tip)
        .build();
}

/// Emits the column of places that are one press away.
#[track_caller]
fn places() {
    rustle::column_arranged(
        Modifier::new()
            .fill_max_height()
            .width(PLACES_WIDTH)
            .border_right_hairline(theme::color(Role::Border))
            .padding_each(0.0, 0.0, 1.0, 0.0)
            .padding(theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(0.5)),
        Alignment::Start,
        || {
            let banded = Modifier::new()
                .height(theme::sp(8))
                .padding_xy(theme::sp(2), 0.0);
            rustle::row_arranged(banded, Arrangement::Start, Alignment::Center, || {
                line("Places", 12, 1.4, Role::Subtle)
            });
            for (index, (name, _, glyph)) in PLACES.iter().enumerate() {
                Tile::new(Target::Dialog(DialogEvent::PickerPlace(index)))
                    .height(8.0)
                    .gap(2.5)
                    .modifier(Modifier::new().fill_max_width())
                    .inner(Modifier::new().padding_xy(theme::sp(2), 0.0))
                    .build_with(|| {
                        icon(glyph.clone(), 4.0, Role::Muted);
                        rustle::basic_text(
                            Modifier::new(),
                            *name,
                            theme::line(13, Role::Text),
                            TextOverflow::Ellipsis,
                        );
                    });
            }
        },
    );
}

/// Emits one folder of the list, the one at `index` called `name`: its glyph and its name on a line, highlighted while it is `selected`, the one the keys act on.
#[track_caller]
fn folder_row(name: &str, selected: bool, index: usize) {
    Tile::new(Target::Dialog(DialogEvent::PickerRow(index)))
        .selected(selected)
        .height(8.0)
        .modifier(Modifier::new().fill_max_width())
        .inner(Modifier::new().padding_xy(theme::sp(2.5), 0.0))
        .build_with(|| {
            icon(phosphor::FOLDER, 4.0, Role::Accent);
            rustle::basic_text(
                Modifier::new().weight(1.0),
                name,
                theme::line(13, Role::Text),
                TextOverflow::Ellipsis,
            );
        });
}

impl WorkspaceState {
    /// Whether the folder picker is open.
    pub fn folder_picker_open(&self) -> bool {
        self.folder_picker.is_some()
    }

    /// What pressing Enter presses while the folder picker is open.
    pub fn folder_picker_enter(&self) -> Option<Target> {
        self.folder_picker.as_ref().map(FolderPicker::enter)
    }

    /// The state with the folder picker open for `purpose` on `machine`, and the server asked for the home folder.
    pub(crate) fn open_folder_picker(&mut self, purpose: Purpose, machine: Option<KnownServer>) {
        let (picker, ask) = FolderPicker::opened(purpose, machine);
        self.folder_picker = Some(picker);
        self.outbox.push(ask);
    }

    /// The state with the folder picker put away.
    pub fn without_folder_picker(mut self) -> Self {
        self.folder_picker = None;
        self
    }

    /// The state after something of the folder picker was pressed: `action` is the name of what.
    pub(crate) fn folder_picker_pressed(mut self, action: Target) -> Self {
        let Some(mut open) = self.folder_picker.take() else {
            return self;
        };
        match open.pressed(action) {
            Outcome::Staying(ask) => {
                self.outbox.extend(ask);
                self.folder_picker = Some(open);
                self
            }
            Outcome::Closed => self,
            Outcome::Chosen(path) => self.folder_chosen(&open, &path),
        }
    }

    /// The state after the folder at `path` was chosen in `picker`: opened as a project, cloned into, or installed from.
    fn folder_chosen(mut self, picker: &FolderPicker, path: &str) -> Self {
        match &picker.purpose {
            Purpose::Open => {
                self = self.with_project_opened(&picker.address(path));
                self.outbox.push(Outgoing::Relist);
            }
            Purpose::CloneInto(repository) => {
                if let Some(open) = self.adder.as_mut() {
                    open.working = Some(format!("Cloning {}", repository.name));
                    open.trouble = None;
                    self.outbox.push(Outgoing::Clone {
                        url: repository.value.clone(),
                        under: path.to_owned(),
                        host: picker.host(),
                    });
                }
            }
            Purpose::Extension => self = self.extension_picked(path.to_owned()),
        }
        self
    }

    /// Takes in what the reader typed in the folder picker's box.
    pub(crate) fn folder_picker_typed(&mut self) {
        if let Some(open) = self.folder_picker.as_mut() {
            open.typed();
        }
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
}

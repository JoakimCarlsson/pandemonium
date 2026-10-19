//! The folder picker: a window of its own for choosing a folder on the machine a server runs on.
//! Places on its left, the folders of the one being looked inside as a list, the path above as a
//! trail with the steps back, forward and up, and the button that takes the folder being looked
//! inside. It is what every part of the app that needs a folder opens, for this machine and for a
//! remote one alike.

use std::sync::atomic::{AtomicU64, Ordering};

use pm_config::KnownServer;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, border_b, border_color, border_r, border_t, flex, flex_1, flex_col, flex_none,
    font_medium, gap, h, items_center, min_h, min_w_px, overflow_hidden, p, px, truncate, w_full,
    w_px,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::adder::{Found, Kind, Listing, Status, status_line};
use crate::button::Variant;
use crate::parts::{glyph, line, spacer};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::tile::Tile;
use crate::{Button, Input, popover};

/// The first name the folder picker answers to.
const PICKER_START: u64 = 21_000_000;

/// Closes the picker without choosing.
pub const PICKER_CLOSE: FocusId = FocusId(21_000_002);

/// Takes the folder being looked inside.
pub const PICKER_CONFIRM: FocusId = FocusId(21_000_003);

/// The box the folders are filtered with.
pub const PICKER_FIELD: FocusId = FocusId(21_000_004);

/// Looks inside the folder that is highlighted.
pub const PICKER_SUBMIT: FocusId = FocusId(21_000_005);

/// Highlights the folder above.
pub const PICKER_UP: FocusId = FocusId(21_000_006);

/// Highlights the folder below.
pub const PICKER_DOWN: FocusId = FocusId(21_000_007);

/// Goes back to the folder looked inside before this one.
pub const PICKER_BACK: FocusId = FocusId(21_000_008);

/// Goes forward again to the folder that was left by going back.
pub const PICKER_FORWARD: FocusId = FocusId(21_000_009);

/// Goes up to the folder that holds the one being looked inside.
pub const PICKER_PARENT: FocusId = FocusId(21_000_010);

/// The scrolling list of folders.
pub const PICKER_SCROLL: FocusId = FocusId(21_000_011);

/// The first of the parts of the path shown above the folders; the rest follow it.
const CRUMB_BASE: u64 = 21_000_100;

/// The first of the places one press away; the rest follow it.
const PLACE_BASE: u64 = 21_000_200;

/// The first of the folders of the list; the rest follow it.
const ROW_BASE: u64 = 21_001_000;

/// One past the last name the folder picker answers to.
const PICKER_END: u64 = 21_100_000;

/// How wide the column of places is, in logical pixels.
const PLACES_WIDTH: f32 = 176.0;

/// How wide the box filtering the folders is, in logical pixels.
const FILTER_WIDTH: f32 = 220.0;

/// How tall a folder of the list is, in logical pixels.
const ROW_HEIGHT: f32 = 32.0;

/// How many times a picker has been opened, which numbers the openings.
static OPENINGS: AtomicU64 = AtomicU64::new(0);

/// The places that are one press away: what they are called, where they are and their glyph.
pub const PLACES: [(&str, &str, IconName); 5] = [
    ("Home", "~", IconName::Folder),
    ("Desktop", "~/Desktop", IconName::Folder),
    ("Documents", "~/Documents", IconName::Folder),
    ("Downloads", "~/Downloads", IconName::Folder),
    ("Root", "/", IconName::Folder),
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
    pub field: Field,
    /// The folders the server listed for the folder being looked inside.
    pub folders: Listing,
    /// The folder being looked inside, as the server gave it.
    pub directory: Option<String>,
    /// The folder the server was last asked about, to tell whether an answer is the one wanted.
    pub requested: String,
    /// The folder that is highlighted, by position; nothing while the list was just entered.
    pub selected: usize,
    /// How far the list is scrolled, in logical pixels.
    pub scroll: f32,
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
pub fn is_picker(id: FocusId) -> bool {
    (PICKER_START..PICKER_END).contains(&id.0)
}

/// Whether every word of `needle` is in `name`, in any case.
fn matches(name: &str, needle: &str) -> bool {
    let name = name.to_lowercase();
    needle
        .to_lowercase()
        .split_whitespace()
        .all(|word| name.contains(word))
}

impl FolderPicker {
    /// The picker for `purpose` looking inside the home folder of `machine`, or of this machine when nothing, with the request that lists it.
    pub fn opened(purpose: Purpose, machine: Option<KnownServer>) -> (Self, Outgoing) {
        let mut picker = Self {
            serial: OPENINGS.fetch_add(1, Ordering::Relaxed),
            purpose,
            machine,
            field: Field::default(),
            folders: Listing::Loading,
            directory: None,
            requested: String::new(),
            selected: usize::MAX,
            scroll: 0.0,
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
        self.field = Field::default();
        self.selected = usize::MAX;
        self.scroll = 0.0;
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

    /// The picker looking inside the folder it was in before, when there is one.
    fn go_back(&mut self) -> Option<Outgoing> {
        let to = self.behind.pop()?;
        if let Some(left) = self.directory.clone() {
            self.ahead.push(left);
        }
        Some(self.visit(&to))
    }

    /// The picker looking inside the folder that going back left, when there is one.
    fn go_forward(&mut self) -> Option<Outgoing> {
        let to = self.ahead.pop()?;
        if let Some(left) = self.directory.clone() {
            self.behind.push(left);
        }
        Some(self.visit(&to))
    }

    /// The picker looking inside the folder that holds the one showing, when it is not the root.
    fn go_up(&mut self) -> Option<Outgoing> {
        let crumbs = trail(self.directory.as_deref()?);
        let (_, parent) = crumbs.get(crumbs.len().checked_sub(2)?)?;
        let parent = parent.clone();
        Some(self.go(&parent))
    }

    /// Whether there is a folder to go back to.
    fn can_go_back(&self) -> bool {
        !self.behind.is_empty()
    }

    /// Whether there is a folder to go forward to.
    fn can_go_forward(&self) -> bool {
        !self.ahead.is_empty()
    }

    /// The folders inside the one being looked into that match what was typed.
    pub fn entries(&self) -> Vec<Found> {
        let Listing::Found(inside) = &self.folders else {
            return Vec::new();
        };
        let typed = self.field.text();
        inside
            .iter()
            .filter(|held| matches(&held.name, typed))
            .cloned()
            .collect()
    }

    /// The line saying what the list is waiting for or why it is empty, when it has no folders to say it.
    fn status(&self) -> Option<Status> {
        if !self.entries().is_empty() {
            return None;
        }
        Some(match &self.folders {
            Listing::Loading => Status::Loading("Loading folders".to_owned()),
            Listing::Failed(reason) => {
                Status::Message(reason.lines().next().unwrap_or(reason).to_owned())
            }
            Listing::Found(inside) if inside.is_empty() => {
                Status::Message("No folders here".to_owned())
            }
            Listing::Found(_) => Status::Message("Nothing matches".to_owned()),
        })
    }

    /// What pressing Enter presses: the folder being looked inside when nothing is typed, and the highlighted folder otherwise.
    pub fn enter(&self) -> FocusId {
        if self.field.text().trim().is_empty() {
            PICKER_CONFIRM
        } else {
            PICKER_SUBMIT
        }
    }

    /// Looks inside the folder of the list at `index`.
    fn open_row(&mut self, index: usize) -> Option<Outgoing> {
        let found = self.entries().get(index).cloned()?;
        Some(self.go(&found.value))
    }

    /// What pressing `id` does to the picker.
    pub fn pressed(&mut self, id: FocusId) -> Outcome {
        match id {
            PICKER_CLOSE => Outcome::Closed,
            PICKER_CONFIRM => match self.directory.clone() {
                Some(path) => Outcome::Chosen(path),
                None => Outcome::Staying(None),
            },
            PICKER_SUBMIT => {
                let index = if self.selected == usize::MAX {
                    0
                } else {
                    self.selected
                };
                Outcome::Staying(self.open_row(index))
            }
            PICKER_UP | PICKER_DOWN => {
                let last = self.entries().len().saturating_sub(1);
                self.selected = match (self.selected, id) {
                    (usize::MAX, _) => 0,
                    (selected, PICKER_UP) => selected.saturating_sub(1),
                    (selected, _) => (selected + 1).min(last),
                };
                self.keep_selected_in_view();
                Outcome::Staying(None)
            }
            PICKER_BACK => Outcome::Staying(self.go_back()),
            PICKER_FORWARD => Outcome::Staying(self.go_forward()),
            PICKER_PARENT => Outcome::Staying(self.go_up()),
            FocusId(number) => Outcome::Staying(self.pressed_part(number)),
        }
    }

    /// What pressing the part of the path, the place or the folder numbered `number` does.
    fn pressed_part(&mut self, number: u64) -> Option<Outgoing> {
        match number {
            n if (CRUMB_BASE..PLACE_BASE).contains(&n) => {
                let crumbs = trail(self.directory.as_deref()?);
                let (_, path) = crumbs.get((n - CRUMB_BASE) as usize)?.clone();
                Some(self.go(&path))
            }
            n if (PLACE_BASE..ROW_BASE).contains(&n) => {
                let (_, path, _) = PLACES.get((n - PLACE_BASE) as usize)?;
                Some(self.go(path))
            }
            n if n >= ROW_BASE => self.open_row((n - ROW_BASE) as usize),
            _ => None,
        }
    }

    /// The picker after `edit` was made to the filter.
    pub fn edited(&mut self, edit: Edit) {
        let before = self.field.text().to_owned();
        self.field.apply(edit);
        if self.field.text() != before {
            self.selected = 0;
            self.scroll = 0.0;
        }
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

    /// The picker with the list scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled(&mut self, delta: f32, room: f32) {
        self.scroll = (self.scroll + delta).clamp(0.0, room);
    }

    /// Scrolls the list so that the highlighted folder is in view.
    fn keep_selected_in_view(&mut self) {
        let top = self.selected as f32 * ROW_HEIGHT;
        let bottom = top + ROW_HEIGHT;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + ROW_HEIGHT * 8.0 {
            self.scroll = bottom - ROW_HEIGHT * 8.0;
        }
    }

    /// What the window holds: the bar with the steps and the path, the places beside the folders, and the bar with the buttons.
    pub fn view(&self) -> Node {
        node()
            .styles([flex(), flex_col(), flex_1(), min_h(0), w_full()])
            .child(self.heading())
            .child(self.bar())
            .child(
                node()
                    .styles([flex(), flex_1(), min_h(0), overflow_hidden()])
                    .child(places())
                    .child(self.list()),
            )
            .child(self.footer())
    }

    /// The line naming what the folder is for, and the machine it is on when that is not this one.
    fn heading(&self) -> Node {
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
            .child(line(self.title(), 13, 1.4, Role::Text))
            .children(
                self.machine
                    .as_ref()
                    .map(|machine| line(format!("on {}", machine.name), 13, 1.4, Role::Subtle)),
            )
    }

    /// The bar above the folders: the steps back, forward and up, the path as a trail that can be pressed anywhere along, and the box filtering the folders.
    fn bar(&self) -> Node {
        let crumbs = self.directory.as_deref().map(trail).unwrap_or_default();
        let last = crumbs.len().saturating_sub(1);
        let mut path = node().styles([flex(), items_center(), gap(0.5), flex_1(), min_w_px(0)]);
        for (index, (name, _)) in crumbs.iter().enumerate() {
            if index > 0 {
                path = path.child(glyph(IconName::ChevronRight, 3.0, Role::Subtle));
            }
            let tone = if index == last {
                Role::Text
            } else {
                Role::Muted
            };
            let mut label = line(name.clone(), 13, 1.4, tone);
            if index == last {
                label = label.styles([font_medium()]);
            }
            path = path.child(
                Tile::new(FocusId(CRUMB_BASE + index as u64))
                    .height(7.0)
                    .build()
                    .styles([px(1.5)])
                    .child(label),
            );
        }
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                h(12),
                px(3),
                flex_none(),
                border_b(),
                border_color(Role::Border),
            ])
            .child(
                Button::icon(IconName::ArrowLeft, PICKER_BACK)
                    .disabled(!self.can_go_back())
                    .build()
                    .tooltip("Back"),
            )
            .child(
                Button::icon(IconName::ArrowRight, PICKER_FORWARD)
                    .disabled(!self.can_go_forward())
                    .build()
                    .tooltip("Forward"),
            )
            .child(
                Button::icon(IconName::ArrowUp, PICKER_PARENT)
                    .disabled(crumbs.len() < 2)
                    .build()
                    .tooltip("Up"),
            )
            .child(path)
            .child(
                node().styles([w_px(FILTER_WIDTH), flex_none()]).child(
                    Input::new(&self.field, PICKER_FIELD)
                        .size(Size::Sm)
                        .placeholder("Filter folders")
                        .build(),
                ),
            )
    }

    /// The folders inside the one being looked into, one to a line, scrolled to where the picker says.
    fn list(&self) -> Node {
        let mut list = node()
            .styles([
                flex(),
                flex_col(),
                gap(0.5),
                p(2),
                flex_1(),
                min_w_px(0),
                min_h(0),
                overflow_hidden(),
            ])
            .scroll_from_start(PICKER_SCROLL, self.scroll);
        list = list.children(
            self.entries()
                .into_iter()
                .enumerate()
                .map(|(index, found)| row(&found.name, index == self.selected, index)),
        );
        if let Some(status) = self.status() {
            list = list.child(status_line(status));
        }
        list
    }

    /// The bar under the folders: the key that closes the picker, and the buttons that close it and that take the folder.
    fn footer(&self) -> Node {
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                h(14),
                px(4),
                flex_none(),
                border_t(),
                border_color(Role::Border),
            ])
            .child(popover::key_hint("Esc", "Close"))
            .child(spacer())
            .child(
                Button::new("Cancel", PICKER_CLOSE)
                    .variant(Variant::Ghost)
                    .build(),
            )
            .child(
                Button::new(self.confirm_label(), PICKER_CONFIRM)
                    .disabled(self.directory.is_none())
                    .build(),
            )
    }
}

/// The column of places that are one press away.
fn places() -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(0.5),
            w_px(PLACES_WIDTH),
            p(2),
            border_r(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), h(8), px(2)])
                .child(line("Places", 12, 1.4, Role::Subtle)),
        )
        .children(PLACES.iter().enumerate().map(|(index, (name, _, icon))| {
            Tile::new(FocusId(PLACE_BASE + index as u64))
                .height(8.0)
                .gap(2.5)
                .build()
                .styles([px(2)])
                .child(glyph(*icon, 4.0, Role::Muted))
                .child(line(*name, 13, 1.4, Role::Text))
        }))
}

/// One folder of the list: its glyph and its name on a line, highlighted while it is the one the keys act on.
fn row(name: &str, selected: bool, index: usize) -> Node {
    Tile::new(FocusId(ROW_BASE + index as u64))
        .selected(selected)
        .height(8.0)
        .build()
        .styles([px(2.5), flex_none()])
        .child(glyph(IconName::Folder, 4.0, Role::Accent))
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(name, 13, 1.4, Role::Text).styles([truncate()])),
        )
}

impl WorkspaceState {
    /// Whether the folder picker is open.
    pub fn folder_picker_open(&self) -> bool {
        self.folder_picker.is_some()
    }

    /// What pressing Enter presses while the folder picker is open.
    pub fn folder_picker_enter(&self) -> Option<FocusId> {
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
    pub(crate) fn folder_picker_pressed(mut self, action: FocusId) -> Self {
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

    /// The state after `edit` was made to the filter of the folder picker.
    pub(crate) fn folder_picker_edited(&mut self, edit: Edit) {
        if let Some(open) = self.folder_picker.as_mut() {
            open.edited(edit);
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

    /// The state with the list of the folder picker scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_folder_picker(mut self, delta: f32, room: f32) -> Self {
        if let Some(open) = self.folder_picker.as_mut() {
            open.scrolled(delta, room);
        }
        self
    }
}

//! Every file of the project as a tree of folders, folders first, with the files git sees changed marked.

use crate::theme;
use crate::theme::sp;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rustle::Modifier;

use crate::Input;
use crate::change_list::mark_color;
use crate::events::Interactive;
use crate::file_icon::{file_icon, folder_icon};
use crate::focus::FocusId;
use crate::icons::icon as icon_of;
use crate::size::Size;
use crate::state::{TREE_BASE, WorkspaceState};
use crate::theme::{Radius, Role, Tone};
use crate::tile::Tile;
use crate::tree::{Entry, Folder, folders};

mod cache;

use crate::tree_actions::{ENTRY_FIELD, Naming, TREE_FILE_BASE, TREE_SPACE, TreeTarget};
use cache::tree;

/// How far, in steps, each level of the tree indents its rows.
const INDENT: f32 = 4.0;

/// The path of every folder of the tree of `files` and the `empty` folders, from the top down, open or not.
pub fn folder_paths(files: &[String], empty: &[String]) -> Vec<String> {
    folders(&tree(files, empty))
        .into_iter()
        .map(|folder| folder.path.clone())
        .collect()
}

/// Every row of the tree of `files` and the `empty` folders shown while the folders in `open` are unfolded, from the top down.
pub fn shown_rows(files: &[String], empty: &[String], open: &HashSet<String>) -> Vec<TreeTarget> {
    /// Adds the rows of `entries`, and of what is in those of their folders that are open, to `rows`.
    fn collect(
        files: &[String],
        entries: &[Entry],
        open: &HashSet<String>,
        rows: &mut Vec<TreeTarget>,
    ) {
        for entry in entries {
            match entry {
                Entry::File { index, .. } => {
                    rows.extend(files.get(*index).map(|path| TreeTarget {
                        path: path.clone(),
                        folder: false,
                    }))
                }
                Entry::Folder(folder) => {
                    rows.push(TreeTarget {
                        path: folder.path.clone(),
                        folder: true,
                    });
                    if open.contains(&folder.path) {
                        collect(files, &folder.entries, open, rows);
                    }
                }
            }
        }
    }
    let mut rows = Vec::new();
    collect(files, &tree(files, empty), open, &mut rows);
    rows
}

/// The letter each changed file of the open session is marked with, by path.
fn marks(state: &WorkspaceState) -> HashMap<&str, &'static str> {
    state.review_session().map_or_else(HashMap::new, |session| {
        session
            .changes
            .iter()
            .map(|change| (change.path.as_str(), change.mark()))
            .collect()
    })
}

/// What a row says about the changes under it: the letter it is marked with, if it is a changed file, and whether it is changed or holds what is.
struct Changed {
    /// The letter git's status is written with, for a file.
    letter: Option<&'static str>,
    /// The colour the name is written in, when there is a change.
    tone: Option<Tone>,
}

impl Changed {
    /// A row with no change under it.
    fn none() -> Self {
        Self {
            letter: None,
            tone: None,
        }
    }

    /// A file or folder git ignores.
    fn ignored() -> Self {
        Self {
            letter: None,
            tone: Some(Tone::from(Role::Muted)),
        }
    }

    /// A file marked `letter`.
    fn file(letter: &'static str) -> Self {
        Self {
            letter: Some(letter),
            tone: Some(mark_color(letter)),
        }
    }

    /// A folder with a changed file somewhere beneath it.
    fn folder() -> Self {
        Self {
            letter: None,
            tone: Some(mark_color("M")),
        }
    }
}

/// How a row is drawn beyond what it names.
struct Look {
    /// What it says about the changes under it.
    changed: Changed,
    /// Whether it is among the rows picked.
    selected: bool,
    /// Whether what the pointer holds would land in it.
    landing: bool,
    /// Whether it was cut and waits to be pasted elsewhere.
    cut: bool,
}

/// How the row of the entry at `path` is drawn, given what it says about the changes under it.
fn look(
    state: &WorkspaceState,
    chosen: &[TreeTarget],
    path: &str,
    folder: bool,
    changed: Changed,
) -> Look {
    Look {
        changed,
        selected: picked(chosen, path, folder),
        landing: folder
            && state
                .git
                .tree_drag
                .as_ref()
                .is_some_and(|drag| drag.into.as_deref() == Some(path)),
        cut: state.git.tree_clipboard.as_ref().is_some_and(|held| {
            held.cut
                && held
                    .targets
                    .iter()
                    .any(|target| target.path == path && target.folder == folder)
        }),
    }
}

/// Emits one row: a name beside `icon`, the icon of what it is, `depth` levels down, with a chevron when `fold` says whether it is an open or closed folder, answering to `id` and standing for `target`, drawn as `look` says, and the letter it is marked with. Clicked with Shift, Control or Command it is picked among others; its menu opens on a secondary press.
#[track_caller]
fn entry_row(
    icon: impl FnOnce(),
    name: &str,
    depth: usize,
    fold: Option<bool>,
    id: FocusId,
    target: TreeTarget,
    look: &Look,
) {
    let operations = crate::events::operations();
    let picked = target.clone();
    let tone = match look.changed.tone {
        _ if look.cut => Tone::from(Role::Subtle),
        Some(tone) => tone,
        None => Tone::from(Role::Text),
    };
    let mut around = Modifier::new().fill_max_width();
    if look.cut {
        around = around.alpha(0.6);
    }
    if look.landing {
        around = around.background_rounded(
            theme::color(Tone::from(Role::Accent).alpha(0.24)),
            Radius::Sm.px(),
        );
    }
    let letter = look.changed.letter;
    Tile::new(id)
        .on_pick(crate::events::picking(id, move |state, how| {
            state.tree_row_picked(picked.clone(), how)
        }))
        .on_context(move || operations.open_tree_menu(target.clone()))
        .selected(look.selected)
        .height(6.0)
        .gap(1.5)
        .radius(Radius::Sm)
        .modifier(crate::bounds::placed(id, around))
        .inner(
            Modifier::new()
                .padding_each(theme::sp(3.0 + depth as f32 * INDENT), 0.0, 0.0, 0.0)
                .padding_each(0.0, 0.0, theme::sp(3), 0.0),
        )
        .build_with(|| {
            match fold {
                Some(true) => icon_of(phosphor::CARET_DOWN, 3.5, Role::Muted),
                Some(false) => icon_of(phosphor::CARET_RIGHT, 3.5, Role::Muted),
                None => rustle::spacer(Modifier::new().size(theme::sp(3.5), theme::sp(3.5))),
            }
            icon();
            rustle::basic_text(
                Modifier::new().weight(1.0),
                name,
                theme::text_style(13, 1.4)
                    .color(theme::color(tone))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
            );
            if let Some(letter) = letter {
                rustle::basic_text(
                    Modifier::new(),
                    letter,
                    theme::mono_style(11, 1.4)
                        .color(theme::color(tone))
                        .max_lines(1),
                    rustle::TextOverflow::Ellipsis,
                );
            }
        });
}

/// Emits what the pointer holds over the tree, named beside it: the row being dragged, or how many there are; nothing while no row is dragged.
#[track_caller]
pub fn drag_ghost(state: &WorkspaceState) {
    let Some(drag) = state.git.tree_drag.as_ref() else {
        return;
    };
    let Some(first) = drag.targets.first() else {
        return;
    };
    let count = drag.targets.len();
    let name = crate::tree_actions::name_of(&first.path);
    let words = if count == 1 {
        name.to_owned()
    } else {
        format!("{count} items")
    };
    row_arranged(
        Modifier::new()
            .offset(drag.at.0 + 12.0, drag.at.1 + 10.0)
            .z_index_unclipped(60)
            .height(theme::sp(7))
            .draw_behind(theme::shadows(&theme::MEDIUM_SHADOW, Radius::Md))
            .background_rounded(theme::color(Role::Surface), Radius::Md.px())
            .border(1.0, theme::color(Role::Border), Radius::Md.px())
            .padding(1.0)
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::SpacedBy(sp(1.5)),
        Alignment::Center,
        || {
            if count != 1 {
                icon_of(phosphor::COPY, 4.0, Role::Muted);
            } else if first.folder {
                folder_icon(false, 4.0);
            } else {
                file_icon(name, 4.0);
            }
            rustle::basic_text(
                Modifier::new(),
                &words,
                theme::text_style(13, 1.4)
                    .color(theme::color(Role::Text))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the field a name is typed into where its row will be, `depth` levels down, with what is wrong with the name under it; nothing while nothing is being named.
#[track_caller]
fn name_lines(state: &WorkspaceState, folder: bool, depth: usize) {
    let Some((_, field)) = state.git.naming.as_ref() else {
        return;
    };
    let problem = state.naming_problem();
    rustle::column_with(Modifier::new().fill_max_width(), || {
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(theme::sp(6))
                .padding_each(theme::sp(3.0 + depth as f32 * INDENT), 0.0, 0.0, 0.0)
                .padding_each(0.0, 0.0, theme::sp(3), 0.0),
            Arrangement::SpacedBy(sp(2)),
            Alignment::Center,
            || {
                rustle::spacer(Modifier::new().size(theme::sp(3.5), theme::sp(3.5)));
                if folder {
                    folder_icon(false, 4.0);
                } else {
                    file_icon(&field.text(), 4.0);
                }
                Input::new(field, ENTRY_FIELD)
                    .size(Size::Sm)
                    .invalid(problem.is_some())
                    .modifier(Modifier::new().weight(1.0))
                    .build();
            },
        );
        let Some(problem) = problem else {
            return;
        };
        column_arranged(
            Modifier::new()
                .padding_each(theme::sp(3.0 + depth as f32 * INDENT + 8.0), 0.0, 0.0, 0.0)
                .padding_each(0.0, 0.0, theme::sp(3), 0.0)
                .padding_each(0.0, 0.0, 0.0, theme::sp(1)),
            Arrangement::Start,
            Alignment::Start,
            || {
                rustle::basic_text(
                    Modifier::new(),
                    problem,
                    theme::text_style(12, 1.4)
                        .color(theme::color(Role::Danger))
                        .max_lines(1),
                    rustle::TextOverflow::Ellipsis,
                )
            },
        );
    });
}

/// What is being named in the tree, if it is a new entry in `directory`: whether it is a folder.
fn new_in(state: &WorkspaceState, directory: &str) -> Option<bool> {
    match state.git.naming.as_ref()? {
        (
            Naming::New {
                directory: at,
                folder,
            },
            _,
        ) if at == directory => Some(*folder),
        _ => None,
    }
}

/// Whether the entry at `path` is the one being renamed.
fn renaming(state: &WorkspaceState, path: &str) -> bool {
    matches!(state.git.naming.as_ref(), Some((Naming::Rename(target), _)) if target.path == path)
}

/// Whether `path` is among the rows `picked`.
fn picked(picked: &[TreeTarget], path: &str, folder: bool) -> bool {
    picked
        .iter()
        .any(|target| target.path == path && target.folder == folder)
}

/// One item of the lazy list of the project's files.
enum Item<'a> {
    /// The field a name is typed into, for a folder or a file, `depth` levels down.
    Field {
        /// Whether a folder is being named.
        folder: bool,
        /// How many levels down the field sits.
        depth: usize,
    },
    /// The row of the file `name`, at `index` of the project's files, `depth` levels down.
    File {
        /// What the file is called.
        name: &'a str,
        /// Its place among the project's files.
        index: usize,
        /// How many levels down the row sits.
        depth: usize,
    },
    /// The row of `folder`, `depth` levels down.
    Folder {
        /// The folder.
        folder: &'a Folder,
        /// How many levels down the row sits.
        depth: usize,
    },
    /// The line saying there are no files.
    Nothing,
    /// The space under the rows that files are dropped on and whose menu acts on the root.
    Space,
}

impl Item<'_> {
    /// The control the item answers to, if it is one focus can be on.
    fn id(&self) -> Option<FocusId> {
        match self {
            Item::Field { .. } => Some(ENTRY_FIELD),
            Item::File { index, .. } => Some(FocusId(TREE_FILE_BASE + *index as u64)),
            Item::Folder { folder, .. } => Some(FocusId(TREE_BASE + folder.position as u64)),
            Item::Nothing => None,
            Item::Space => Some(TREE_SPACE),
        }
    }

    /// The key the list knows the item by.
    fn key(&self) -> u64 {
        self.id().map_or(u64::MAX, |id| id.0)
    }

    /// The row of the tree the item is, if it is one.
    fn row(&self) -> Option<FocusId> {
        match self {
            Item::File { .. } | Item::Folder { .. } => self.id(),
            _ => None,
        }
    }
}

/// Every item of the list of the project's files in `state`, from the top down: the rows of
/// the open folders, the field a name is typed into where its row goes, the line saying there
/// are none, and the space under them.
fn items<'a>(state: &'a WorkspaceState, entries: &'a [Entry]) -> Vec<Item<'a>> {
    /// Adds the items of `entries`, `depth` levels down, and of what their open folders hold.
    fn collect<'a>(
        state: &'a WorkspaceState,
        entries: &'a [Entry],
        depth: usize,
        items: &mut Vec<Item<'a>>,
    ) {
        for entry in entries {
            match entry {
                Entry::File { name, index } => {
                    let path = state.files.get(*index).map_or("", String::as_str);
                    items.push(if renaming(state, path) {
                        Item::Field {
                            folder: false,
                            depth,
                        }
                    } else {
                        Item::File {
                            name,
                            index: *index,
                            depth,
                        }
                    });
                }
                Entry::Folder(folder) => {
                    items.push(if renaming(state, &folder.path) {
                        Item::Field {
                            folder: true,
                            depth,
                        }
                    } else {
                        Item::Folder { folder, depth }
                    });
                    if state.open_tree.contains(&folder.path) {
                        if let Some(is_folder) = new_in(state, &folder.path) {
                            items.push(Item::Field {
                                folder: is_folder,
                                depth: depth + 1,
                            });
                        }
                        collect(state, &folder.entries, depth + 1, items);
                    }
                }
            }
        }
    }
    let mut items = Vec::new();
    if let Some(is_folder) = new_in(state, "") {
        items.push(Item::Field {
            folder: is_folder,
            depth: 0,
        });
    }
    collect(state, entries, 0, &mut items);
    if state.files.is_empty() && state.tree_folders.is_empty() {
        items.push(Item::Nothing);
    }
    items.push(Item::Space);
    items
}

/// Scrolls `list` the least that brings item `index` wholly into view, as of its latest
/// layout: to the item's top when it is above the view, to its bottom when below, and not at
/// all when it is in view already.
fn keep_in_view(list: &rustle::LazyListState, index: usize) {
    let info = list.layout_info();
    let seen = info
        .visible_items_info
        .iter()
        .find(|item| item.index == index);
    match seen {
        Some(item)
            if item.offset >= info.viewport_start_offset
                && item.offset + item.size <= info.viewport_end_offset => {}
        Some(item) if item.offset < info.viewport_start_offset => {
            list.scroll_to_item(index, rustle::ItemAlign::Start)
        }
        Some(_) => list.scroll_to_item(index, rustle::ItemAlign::End),
        None if info
            .visible_items_info
            .first()
            .is_some_and(|first| index < first.index) =>
        {
            list.scroll_to_item(index, rustle::ItemAlign::Start)
        }
        None => list.scroll_to_item(index, rustle::ItemAlign::End),
    }
}

impl WorkspaceState {
    /// Scrolls the list of the project's files the least that shows the row of `target`, or
    /// the field a name is typed into when `target` is being renamed or `target` is the folder
    /// a new entry is named in.
    pub(crate) fn keep_tree_row_in_view(&self, target: &TreeTarget) {
        let entries = tree(&self.files, &self.tree_folders);
        let items = items(self, &entries);
        let naming_here =
            renaming(self, &target.path) || (target.folder && new_in(self, &target.path).is_some());
        let found = items.iter().position(|item| match item {
            Item::Field { .. } => naming_here,
            Item::File { index, .. } if !naming_here && !target.folder => self
                .files
                .get(*index)
                .is_some_and(|path| *path == target.path),
            Item::Folder { folder, .. } if !naming_here && target.folder => {
                folder.path == target.path
            }
            _ => false,
        });
        if let Some(at) = found {
            keep_in_view(&self.files_list, at);
        }
    }
}

/// The key handler of the tree: Tab or Shift+Tab from a row moves to the next or previous row
/// in tree order, scrolling it wholly into view and asking for focus on it, as a lazy list
/// brings the item focus moves to into view; past the first or last row Tab is left alone.
fn walks_rows(rows: Rc<Vec<(FocusId, usize)>>, list: rustle::LazyListState) -> Modifier {
    let keys = crate::focus::keys();
    Modifier::new().on_key_event(move |event| {
        if !event.pressed || event.key != rustle::Key::Tab {
            return false;
        }
        let Some(at) = keys
            .held
            .and_then(|held| rows.iter().position(|(id, _)| *id == held))
        else {
            return false;
        };
        let next = if event.modifiers.shift {
            at.checked_sub(1)
        } else {
            Some(at + 1).filter(|next| *next < rows.len())
        };
        let Some((id, index)) = next.map(|next| rows[next]) else {
            return false;
        };
        keep_in_view(&list, index);
        (keys.request)(id);
        true
    })
}

/// Keeps the item being composed composed while focus is on `id`, even scrolled out of view,
/// as a focused item of a lazy list pins itself.
#[track_caller]
fn pin_while_held(id: FocusId) {
    let held = crate::focus::is_held(id);
    let container = rustle::LOCAL_PINNABLE_CONTAINER.current();
    rustle::disposable_effect(held, move || {
        let handle = container.filter(|_| held).map(|container| container.pin());
        move || drop(handle)
    });
}

/// Emits the space under the rows: as tall as what the rows leave of the view, and
/// [`SPACE_LEAST`] steps at the least, lit while what the pointer holds would land in the root.
#[track_caller]
fn space(state: &WorkspaceState, index: usize) {
    let info = state.files_list.layout_info();
    let least = sp(SPACE_LEAST);
    let height = info
        .visible_items_info
        .iter()
        .find(|item| item.index == index)
        .map_or(least, |item| {
            (info.viewport_end_offset - item.offset - sp(1)).max(least)
        });
    let landing = state
        .git
        .tree_drag
        .as_ref()
        .is_some_and(|drag| drag.into.as_deref() == Some(""));
    let mut around = Modifier::new().fill_max_width().height(height);
    if landing {
        around = around.background_rounded(
            theme::color(Tone::from(Role::Accent).alpha(0.24)),
            Radius::Sm.px(),
        );
    }
    let operations = crate::events::operations();
    let pressed = crate::events::reduce(TREE_SPACE);
    rustle::spacer(crate::bounds::placed(
        TREE_SPACE,
        around.control_menu(
            TREE_SPACE,
            move |_| pressed(),
            move || operations.open_tree_menu(TreeTarget::root()),
        ),
    ));
}

/// How tall, in steps, the space under the files is at the least.
pub(crate) const SPACE_LEAST: f32 = 32.0;

/// Emits the row of the file `name` at `index` of the project's files, `depth` levels down.
#[track_caller]
fn file_row(
    state: &WorkspaceState,
    marks: &HashMap<&str, &'static str>,
    chosen: &[TreeTarget],
    name: &str,
    index: usize,
    depth: usize,
) {
    let path = state.files.get(index).map_or("", String::as_str);
    let changed = match marks.get(path) {
        Some(letter) => Changed::file(letter),
        None if state.tree_ignored.contains(path) => Changed::ignored(),
        None => Changed::none(),
    };
    entry_row(
        || file_icon(name, 4.0),
        name,
        depth,
        None,
        FocusId(TREE_FILE_BASE + index as u64),
        TreeTarget {
            path: path.to_owned(),
            folder: false,
        },
        &look(state, chosen, path, false, changed),
    );
}

/// Emits the row of `folder`, `depth` levels down.
#[track_caller]
fn folder_row(
    state: &WorkspaceState,
    marks: &HashMap<&str, &'static str>,
    chosen: &[TreeTarget],
    folder: &Folder,
    depth: usize,
) {
    let open = state.open_tree.contains(&folder.path);
    let changed = if folder.files.iter().any(|index| {
        state
            .files
            .get(*index)
            .is_some_and(|path| marks.contains_key(path.as_str()))
    }) {
        Changed::folder()
    } else if state.tree_ignored.contains(&folder.path) {
        Changed::ignored()
    } else {
        Changed::none()
    };
    entry_row(
        || folder_icon(open, 4.0),
        &folder.name,
        depth,
        Some(open),
        FocusId(TREE_BASE + folder.position as u64),
        TreeTarget {
            path: folder.path.clone(),
            folder: true,
        },
        &look(state, chosen, &folder.path, true, changed),
    );
}

/// Emits the project's files of the open session as a lazy list filling what is left of its
/// column: the rows of the open folders, the field a name is typed in where it goes, and the
/// space under them, composing only the items in view and the one focus is on.
#[track_caller]
pub fn list(state: &WorkspaceState) {
    let entries = tree(&state.files, &state.tree_folders);
    let items = items(state, &entries);
    let rows: Rc<Vec<(FocusId, usize)>> = Rc::new(
        items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.row().map(|id| (id, index)))
            .collect(),
    );
    let marks = marks(state);
    let chosen = state.tree_picked();
    let keys = walks_rows(rows, state.files_list.clone());
    rustle::lazy_list_with(
        Modifier::new().fill_max_width().weight(1.0).chain(keys),
        &state.files_list,
        rustle::LazyContent::new().padding(sp(1)),
        items.len(),
        |index| items[index].key(),
        |index| {
            let item = &items[index];
            if let Some(id) = item.id() {
                pin_while_held(id);
            }
            match item {
                Item::Field { folder, depth } => {
                    name_lines(state, *folder, *depth);
                }
                Item::File { name, index, depth } => {
                    file_row(state, &marks, &chosen, name, *index, *depth)
                }
                Item::Folder { folder, depth } => {
                    folder_row(state, &marks, &chosen, folder, *depth)
                }
                Item::Nothing => crate::review::nothing(if state.files_loading() {
                    "Loading files…"
                } else {
                    "No files yet."
                }),
                Item::Space => space(state, index),
            }
        },
    );
}

#[cfg(test)]
mod tests;

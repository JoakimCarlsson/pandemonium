//! Every file of the project as a tree of folders, folders first, with the files git sees changed marked.

use crate::areas::TreeEvent;
use crate::icons;
use crate::theme::Frame;
use crate::theme::ShadowLayers;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::pointing::Over;
use crate::target::Target;
use crate::theme;
use crate::theme::sp;
use rustle::phosphor;

use std::collections::{HashMap, HashSet};

use rustle::{Color, Modifier};

use crate::Input;
use crate::change_list::mark_color;
use crate::events::Interactive;
use crate::file_icon::{file_icon, folder_icon};
use crate::icons::icon as icon_of;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::theme::{Radius, Role};
use crate::tile::Tile;
use crate::tree::{Entry, Folder, folders};

mod cache;

use crate::tree_actions::{Naming, TreeTarget};
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
    tone: Option<Color>,
}

impl Changed {
    /// A row marked with no letter, its name written in `tone` when it is given one.
    fn unlettered(tone: Option<Color>) -> Self {
        Self { letter: None, tone }
    }

    /// The row of the entry at `path` with no change under it, muted when git ignores it.
    fn unchanged(state: &WorkspaceState, path: &str) -> Self {
        Self::unlettered(
            state
                .tree_ignored
                .contains(path)
                .then(|| Color::from(Role::Muted)),
        )
    }

    /// A file marked `letter`.
    fn file(letter: &'static str) -> Self {
        Self {
            letter: Some(letter),
            tone: Some(mark_color(letter)),
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

/// The modifier of the row of the project's files standing for `target`, or of the space
/// under them standing for their root, as a place a carried row and files from another
/// application can be let go over.
fn tree_place(target: TreeTarget) -> Modifier {
    crate::events::carried_over(true, move |_, _| Over::Tree(target.clone()))
}

/// Emits one row: a name beside `icon`, the icon of what it is, `depth` levels down, with a chevron when `fold` says whether it is an open or closed folder, answering to `id` and standing for `target`, drawn as `look` says, and the letter it is marked with. Clicked with Shift, Control or Command it is picked among others; its menu opens on a secondary press.
#[track_caller]
fn entry_row(
    icon: impl FnOnce(),
    name: &str,
    depth: usize,
    fold: Option<bool>,
    id: Target,
    target: TreeTarget,
    look: &Look,
) {
    let operations = crate::events::operations();
    let picked = target.clone();
    let place = tree_place(target.clone());
    let tone = match look.changed.tone {
        _ if look.cut => Color::from(Role::Subtle),
        Some(tone) => tone,
        None => Color::from(Role::Text),
    };
    let mut around = Modifier::new().fill_max_width();
    if look.cut {
        around = around.alpha(0.6);
    }
    if look.landing {
        around = around.background_rounded(theme::color(Role::Accent.alpha(0.24)), Radius::Sm.px());
    }
    let letter = look.changed.letter;
    Tile::new(id)
        .on_pick(crate::events::picking(id, move |state, how| {
            state.tree_row_picked(picked.clone(), how)
        }))
        .on_context(move |at| operations.open_tree_menu(target.clone(), at))
        .selected(look.selected)
        .height(6.0)
        .gap(1.5)
        .radius(Radius::Sm)
        .modifier(crate::bounds::placed(id, around).chain(place))
        .inner(Modifier::new().padding_each(
            theme::sp(3.0 + depth as f32 * INDENT),
            0.0,
            theme::sp(3),
            0.0,
        ))
        .build_with(|| {
            match fold {
                Some(open) => icon_of(icons::chevron(open), 3.5, Role::Muted),
                None => rustle::spacer(Modifier::new().size(theme::sp(3.5), theme::sp(3.5))),
            }
            icon();
            rustle::basic_text(
                Modifier::new().weight(1.0),
                name,
                theme::line(13, tone),
                TextOverflow::Ellipsis,
            );
            if let Some(letter) = letter {
                rustle::basic_text(
                    Modifier::new(),
                    letter,
                    theme::code(11, tone),
                    TextOverflow::Ellipsis,
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
    rustle::row_arranged(
        Modifier::new()
            .offset(drag.at.0 + 12.0, drag.at.1 + 10.0)
            .z_index_unclipped(60)
            .height(theme::sp(7))
            .shadow_layers(&theme::MEDIUM_SHADOW, Radius::Md.px())
            .panel(Radius::Md)
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::SpacedBy(theme::sp(1.5)),
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
                theme::line(13, Role::Text),
                TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the field a name is typed into where its row will be, `depth` levels down, with what is wrong with the name under it; nothing while nothing is being named.
fn name_lines(state: &WorkspaceState, folder: bool, depth: usize) {
    let Some((_, field)) = state.git.naming.as_ref() else {
        return;
    };
    let problem = state.naming_problem();
    rustle::column_with(Modifier::new().fill_max_width(), || {
        rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(theme::sp(6))
                .padding_each(
                    theme::sp(3.0 + depth as f32 * INDENT),
                    0.0,
                    theme::sp(3),
                    0.0,
                ),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                rustle::spacer(Modifier::new().size(theme::sp(3.5), theme::sp(3.5)));
                if folder {
                    folder_icon(false, 4.0);
                } else {
                    file_icon(&field.text(), 4.0);
                }
                crate::focus::request_focus_on(Target::Tree(TreeEvent::EntryField), field);
                Input::new(field, Target::Tree(TreeEvent::EntryField))
                    .size(Size::Sm)
                    .invalid(problem.is_some())
                    .modifier(Modifier::new().weight(1.0))
                    .submits(Target::Tree(TreeEvent::TreeNameSubmit))
                    .build();
            },
        );
        let Some(problem) = problem else {
            return;
        };
        rustle::column_arranged(
            Modifier::new().padding_each(
                theme::sp(3.0 + depth as f32 * INDENT + 8.0),
                0.0,
                theme::sp(3),
                theme::sp(1),
            ),
            Arrangement::Start,
            Alignment::Start,
            || {
                rustle::basic_text(
                    Modifier::new(),
                    problem,
                    theme::line(12, Role::Danger),
                    TextOverflow::Ellipsis,
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
    fn id(&self) -> Option<Target> {
        match self {
            Item::Field { .. } => Some(Target::Tree(TreeEvent::EntryField)),
            Item::File { index, .. } => Some(Target::Tree(TreeEvent::TreeFile(*index))),
            Item::Folder { folder, .. } => Some(Target::Tree(TreeEvent::Tree(folder.position))),
            Item::Nothing => None,
            Item::Space => Some(Target::Tree(TreeEvent::TreeSpace)),
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
                    let index = *index;
                    items.push(match renaming(state, path) {
                        true => Item::Field {
                            folder: false,
                            depth,
                        },
                        false => Item::File { name, index, depth },
                    });
                }
                Entry::Folder(folder) => {
                    items.push(match renaming(state, &folder.path) {
                        true => Item::Field {
                            folder: true,
                            depth,
                        },
                        false => Item::Folder { folder, depth },
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
            crate::rows_in_view::keep_in_view(&self.files_list, at);
        }
    }
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
        around = around.background_rounded(theme::color(Role::Accent.alpha(0.24)), Radius::Sm.px());
    }
    let operations = crate::events::operations();
    let pressed = crate::events::send(crate::events::Event::Tree(TreeEvent::TreeSpace));
    rustle::spacer(crate::bounds::placed(
        Target::Tree(TreeEvent::TreeSpace),
        around.chain(tree_place(TreeTarget::root())).control_menu(
            Target::Tree(TreeEvent::TreeSpace),
            move |_| pressed(),
            move |at| operations.open_tree_menu(TreeTarget::root(), at),
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
        None => Changed::unchanged(state, path),
    };
    entry_row(
        || file_icon(name, 4.0),
        name,
        depth,
        None,
        Target::Tree(TreeEvent::TreeFile(index)),
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
        Changed::unlettered(Some(mark_color("M")))
    } else {
        Changed::unchanged(state, &folder.path)
    };
    entry_row(
        || folder_icon(open, 4.0),
        &folder.name,
        depth,
        Some(open),
        Target::Tree(TreeEvent::Tree(folder.position)),
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
    let marks = marks(state);
    let chosen = state.tree_picked();
    rustle::lazy_list_with(
        Modifier::new().fill_max_width().weight(1.0),
        &state.files_list,
        rustle::LazyContent::new().padding(sp(1)),
        items.len(),
        |index| items[index].id(),
        |index| match &items[index] {
            Item::Field { folder, depth } => {
                name_lines(state, *folder, *depth);
            }
            Item::File { name, index, depth } => {
                file_row(state, &marks, &chosen, name, *index, *depth)
            }
            Item::Folder { folder, depth } => folder_row(state, &marks, &chosen, folder, *depth),
            Item::Nothing => crate::review::nothing(if state.files_loading() {
                "Loading files…"
            } else {
                "No files yet."
            }),
            Item::Space => space(state, index),
        },
    );
}

#[cfg(test)]
mod tests;

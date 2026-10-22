//! The changed files in groups: staged, tracked and untracked, each file with how it changed, a way to open it and a box that stages it.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, flex, flex_1, flex_none, font_mono, items_center, justify_between,
    leading_ratio, min_w_px, overflow_hidden, pb, pl, pr, pt, text_color, text_px, truncate,
};
use pm_ui::{Node, node, text};
use std::collections::HashSet;

use crate::file_icon::file_icon;
use crate::git_controls::{GROUP_TRACKED, GROUP_UNTRACKED, OPEN_ARROW_BASE};
use crate::parts::{glyph, line};
use crate::sample::{Change, Session};
use crate::size::Size;
use crate::state::{FILE_BASE, FOLDER_BASE, FOLDER_STAGE_BASE, STAGE_BASE, WorkspaceState};
use crate::tile::Tile;
use crate::tree::{Entry, Folder, Layout, build_stopping, folders};
use crate::{Button, Checkbox, Tally};

/// Where a changed file is listed.
///
/// Staging a file leaves it in the group it was in: what the box on its row says is how much of it is in the index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    /// Changed on both sides of a merge.
    Conflicted,
    /// Changed, and git knows the file.
    Tracked,
    /// New, and git has never been told about it.
    Untracked,
}

impl Group {
    /// The groups in the order they are listed.
    const ALL: [Self; 3] = [Self::Conflicted, Self::Tracked, Self::Untracked];

    /// What the heading of the group says.
    fn label(self) -> &'static str {
        match self {
            Self::Conflicted => "CONFLICTS",
            Self::Tracked => "TRACKED",
            Self::Untracked => "UNTRACKED",
        }
    }

    /// The name the box on the heading answers to, when the group has one.
    fn id(self) -> Option<FocusId> {
        match self {
            Self::Conflicted => None,
            Self::Tracked => Some(GROUP_TRACKED),
            Self::Untracked => Some(GROUP_UNTRACKED),
        }
    }

    /// Whether `change` is listed in this group.
    fn holds(self, change: &Change) -> bool {
        match self {
            Self::Conflicted => change.conflicted,
            Self::Tracked => !change.conflicted && !change.untracked,
            Self::Untracked => !change.conflicted && change.untracked,
        }
    }
}

/// The positions in `changes` of the files of `group` that are in `repository`, a submodule or nothing for the folder's own repository, in the order they came in.
pub fn members(changes: &[Change], group: Group, repository: Option<&str>) -> Vec<usize> {
    changes
        .iter()
        .enumerate()
        .filter(|(_, change)| group.holds(change) && change.submodule.as_deref() == repository)
        .map(|(index, _)| index)
        .collect()
}

/// The colour the letter that says how `change` came to differ is written in.
pub(crate) fn mark_color(letter: &str) -> Color {
    match letter {
        "A" | "U" => Color::from(Role::Success),
        "D" => Color::from(Role::Subtle),
        "C" => Color::from(Role::Danger),
        _ => Color::rgb(0xe6, 0xc8, 0x4f),
    }
}

/// How much of a file is in the index, as its box says.
fn staged_box(change: &Change, id: FocusId) -> Node {
    Checkbox::new(change.fully_staged(), id)
        .mixed(change.staged && change.unstaged)
        .build()
}

/// The heading above the files of `group` in `repository`, and the box that stages them all: none of them, all of them or some.
fn heading(changes: &[Change], group: Group, repository: Option<&str>) -> Node {
    let files = members(changes, group, repository);
    let staged = files
        .iter()
        .filter(|&&index| changes[index].fully_staged())
        .count();
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            pl(3),
            pr(3),
            pt(2),
            pb(1),
        ])
        .child(
            text(format!("{} ({})", group.label(), files.len())).styles([
                text_px(11),
                leading_ratio(1.4),
                text_color(Role::Subtle),
            ]),
        );
    if let Some(id) = group.id() {
        row = row.child(
            Checkbox::new(staged == files.len(), id)
                .mixed(staged > 0 && staged < files.len())
                .build(),
        );
    }
    row
}

/// How far, in steps, each level of a group's tree indents its rows.
const INDENT: f32 = 3.5;

/// How far, in steps, a file is set in beyond the folder it is in, to line its icon up under the folder's name.
const FILE_OFFSET: f32 = 5.5;

/// A folder of the tree of one group, with the files beneath it.
pub struct GroupFolder {
    /// The group whose tree it is in.
    pub group: Group,
    /// Where the folder is, from the root of the repository.
    pub path: String,
    /// The positions in the changes of every file beneath it.
    pub files: Vec<usize>,
}

/// `path` as it is from `repository`, a submodule or nothing for the folder's own repository.
fn relative<'a>(path: &'a str, repository: Option<&str>) -> &'a str {
    repository
        .and_then(|repository| path.strip_prefix(repository))
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(path)
}

/// Where `change` is listed in the tree of `repository`: at its path from there, or, for a submodule moved to another commit, as a row inside the submodule's own folder.
fn tree_path(change: &Change, repository: Option<&str>) -> String {
    let path = relative(&change.path, repository);
    match change.moved_label() {
        Some(label) => format!("{path}/{label}"),
        None => path.to_owned(),
    }
}

/// The folders of `changes` that are submodules moved to another commit, from `repository`.
fn submodule_folders<'a>(changes: &'a [Change], repository: Option<&str>) -> Vec<&'a str> {
    changes
        .iter()
        .filter(|change| change.moved.is_some())
        .map(|change| relative(&change.path, repository))
        .collect()
}

/// The tree of the files of `group` in `repository`: folders that hold only a folder are joined into one row, except that a submodule's row always ends at it.
fn tree(changes: &[Change], members: &[usize], repository: Option<&str>) -> Vec<Entry> {
    let paths: Vec<String> = members
        .iter()
        .map(|&index| tree_path(&changes[index], repository))
        .collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    build_stopping(
        &paths,
        Layout {
            compact: true,
            sorted: false,
        },
        &submodule_folders(changes, repository),
    )
}

/// Every folder of every group's tree of `repository`, group by group and from the top down, folded or not: the place of a folder in this list is what its row answers to.
pub fn folder_table(changes: &[Change], repository: Option<&str>) -> Vec<GroupFolder> {
    let mut table = Vec::new();
    for group in Group::ALL {
        let files = members(changes, group, repository);
        for folder in folders(&tree(changes, &files, repository)) {
            table.push(GroupFolder {
                group,
                path: folder.path.clone(),
                files: folder.files.iter().map(|&place| files[place]).collect(),
            });
        }
    }
    table
}

/// The key a folded folder is kept under: which group's tree it is in, and where it is.
pub fn fold_key(group: Group, path: &str) -> String {
    format!("{group:?}:{path}")
}

/// The positions in `changes` of the files of `repository` the list shows, top to bottom, leaving out those in folders folded away in `closed`.
pub fn order_shown(
    changes: &[Change],
    closed: &HashSet<String>,
    repository: Option<&str>,
) -> Vec<usize> {
    fn collect(
        entries: &[Entry],
        group: Group,
        files: &[usize],
        closed: &HashSet<String>,
        order: &mut Vec<usize>,
    ) {
        for entry in entries {
            match entry {
                Entry::File { index, .. } => order.push(files[*index]),
                Entry::Folder(folder) if !closed.contains(&fold_key(group, &folder.path)) => {
                    collect(&folder.entries, group, files, closed, order);
                }
                Entry::Folder(_) => {}
            }
        }
    }
    let mut order = Vec::new();
    for group in Group::ALL {
        let files = members(changes, group, repository);
        collect(
            &tree(changes, &files, repository),
            group,
            &files,
            closed,
            &mut order,
        );
    }
    order
}

/// How many commits a submodule moved by, and which way.
fn commit_count(moved: &pm_git::Moved) -> String {
    let count = moved.commits.len();
    let noun = if count == 1 { "commit" } else { "commits" };
    if moved.back {
        format!("{count} {noun} back")
    } else {
        format!("{count} {noun}")
    }
}

/// The row of the file at `index`, `depth` levels down: how it changed, a button that opens it and the box that stages it.
fn file_row(
    state: &WorkspaceState,
    change: &Change,
    index: usize,
    name: &str,
    depth: usize,
) -> Node {
    let letter = change.mark();
    let tone = mark_color(letter);
    let icon = match &change.moved {
        Some(_) => glyph(IconName::Git, 3.5, Role::Muted),
        None => file_icon(name, 3.5),
    };
    Tile::new(FocusId(FILE_BASE + index as u64))
        .selected(state.git.marked.contains(&index))
        .height(6.5)
        .gap(2.0)
        .radius(Radius::Sm)
        .build()
        .styles([
            pl(3.0 + depth as f32 * INDENT + FILE_OFFSET),
            pr(3),
            flex_none(),
            overflow_hidden(),
        ])
        .child(icon)
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(name, 13, 1.4, Role::Text).styles([truncate()])),
        )
        .children(
            change.moved.as_ref().map(|moved| {
                line(commit_count(moved), 11, 1.4, Role::Subtle).styles([flex_none()])
            }),
        )
        .child(text(letter).styles([
            text_px(11),
            leading_ratio(1.4),
            font_mono(),
            text_color(tone),
            flex_none(),
        ]))
        .children(change.moved.is_none().then(|| {
            Button::icon(
                IconName::ArrowRight,
                FocusId(OPEN_ARROW_BASE + index as u64),
            )
            .size(Size::Sm)
            .build()
        }))
        .children(
            (!change.conflicted).then(|| staged_box(change, FocusId(STAGE_BASE + index as u64))),
        )
}

/// The row of a folder, `depth` levels down, open or folded: a chevron, the box that stages everything in it, its name and what it changed.
fn folder_row(
    changes: &[Change],
    folder: &Folder,
    files: &[usize],
    position: usize,
    depth: usize,
    open: bool,
) -> Node {
    let inside: Vec<usize> = folder.files.iter().map(|&place| files[place]).collect();
    let staged = inside
        .iter()
        .filter(|&&index| changes[index].fully_staged())
        .count();
    let partly = inside.iter().any(|&index| changes[index].staged);
    let added = inside.iter().map(|&index| changes[index].added()).sum();
    let removed = inside.iter().map(|&index| changes[index].removed()).sum();
    Tile::new(FocusId(FOLDER_BASE + position as u64))
        .height(6.5)
        .gap(2.0)
        .radius(Radius::Sm)
        .build()
        .styles([pl(3.0 + depth as f32 * INDENT), pr(3), flex_none()])
        .child(glyph(
            if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            },
            3.5,
            Role::Muted,
        ))
        .child(
            Checkbox::new(
                !inside.is_empty() && staged == inside.len(),
                FocusId(FOLDER_STAGE_BASE + position as u64),
            )
            .mixed(partly && staged < inside.len())
            .build(),
        )
        .children(
            submodule_folders(changes, None)
                .contains(&folder.path.as_str())
                .then(|| glyph(IconName::Git, 3.5, Role::Muted)),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(&folder.name, 13, 1.4, Role::Text).styles([truncate()])),
        )
        .child(Tally::new(added, removed).size(11).build())
}

/// The rows of `entries` and of everything in their open folders, `depth` levels down; `counter` numbers the folders across the groups as [`folder_table`] does.
fn rows_of(
    state: &WorkspaceState,
    changes: &[Change],
    entries: &[Entry],
    group: Group,
    files: &[usize],
    depth: usize,
    counter: &mut usize,
) -> Vec<Node> {
    let mut rows = Vec::new();
    for entry in entries {
        match entry {
            Entry::File { name, index } => {
                let at = files[*index];
                rows.push(file_row(state, &changes[at], at, name, depth));
            }
            Entry::Folder(folder) => {
                let position = *counter;
                *counter += 1;
                let open = !state
                    .closed_folders
                    .contains(&fold_key(group, &folder.path));
                rows.push(folder_row(changes, folder, files, position, depth, open));
                let inner = rows_of(
                    state,
                    changes,
                    &folder.entries,
                    group,
                    files,
                    depth + 1,
                    counter,
                );
                if open {
                    rows.extend(inner);
                }
            }
        }
    }
    rows
}

/// The rows of the changes of `session` in the repository the panel works on: each group that has files, under its heading, as a tree of folders.
pub fn rows(state: &WorkspaceState, session: &Session) -> Vec<Node> {
    let repository = state.git.repository.as_deref();
    let mut rows = Vec::new();
    let mut counter = 0;
    for group in Group::ALL {
        let files = members(&session.changes, group, repository);
        if files.is_empty() {
            continue;
        }
        rows.push(heading(&session.changes, group, repository));
        rows.extend(rows_of(
            state,
            &session.changes,
            &tree(&session.changes, &files, repository),
            group,
            &files,
            0,
            &mut counter,
        ));
    }
    rows
}

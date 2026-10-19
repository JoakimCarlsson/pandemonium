//! The changed files in groups: staged, tracked and untracked, each file with how it changed, a way to open it and a box that stages it.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Role, bg, cursor_pointer, flex, flex_1, flex_none, font_mono, gap, h, items_center,
    justify_between, leading_ratio, min_w_px, overflow_hidden, pb, pl, pr, pt, text_color, text_px,
    truncate,
};
use pm_ui::{Node, node, text};
use std::collections::HashSet;

use crate::file_icon::file_icon;
use crate::git_controls::{GROUP_TRACKED, GROUP_UNTRACKED, OPEN_ARROW_BASE};
use crate::parts::{glyph, line};
use crate::sample::{Change, Session};
use crate::size::Size;
use crate::state::{FILE_BASE, FOLDER_BASE, FOLDER_STAGE_BASE, STAGE_BASE, WorkspaceState};
use crate::tree::{Entry, Folder, Layout, build, folders};
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

/// The positions in `changes` of the files of `group`, in the order they came in.
pub fn members(changes: &[Change], group: Group) -> Vec<usize> {
    changes
        .iter()
        .enumerate()
        .filter(|(_, change)| group.holds(change))
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

/// The heading above the files of `group`, and the box that stages them all: none of them, all of them or some.
fn heading(changes: &[Change], group: Group) -> Node {
    let files = members(changes, group);
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

/// The tree of the files of `group`: folders that hold only a folder are joined into one row.
fn tree(changes: &[Change], members: &[usize]) -> Vec<Entry> {
    let paths: Vec<&str> = members
        .iter()
        .map(|&index| changes[index].path.as_str())
        .collect();
    build(
        &paths,
        Layout {
            compact: true,
            sorted: false,
        },
    )
}

/// Every folder of every group's tree, group by group and from the top down, folded or not: the place of a folder in this list is what its row answers to.
pub fn folder_table(changes: &[Change]) -> Vec<GroupFolder> {
    let mut table = Vec::new();
    for group in Group::ALL {
        let files = members(changes, group);
        for folder in folders(&tree(changes, &files)) {
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

/// The positions in `changes` of the files the list shows, top to bottom, leaving out those in folders folded away in `closed`.
pub fn order_shown(changes: &[Change], closed: &HashSet<String>) -> Vec<usize> {
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
        let files = members(changes, group);
        collect(&tree(changes, &files), group, &files, closed, &mut order);
    }
    order
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
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(6.5),
        pl(3.0 + depth as f32 * INDENT + FILE_OFFSET),
        pr(3),
        flex_none(),
        overflow_hidden(),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if state.git.marked.contains(&index) {
        styles.push(bg(Role::SurfaceSelected));
    }
    node()
        .styles(styles)
        .child(file_icon(name, 3.5))
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(name, 13, 1.4, Role::Text).styles([truncate()])),
        )
        .child(text(letter).styles([
            text_px(11),
            leading_ratio(1.4),
            font_mono(),
            text_color(tone),
            flex_none(),
        ]))
        .child(
            Button::icon(
                IconName::ArrowRight,
                FocusId(OPEN_ARROW_BASE + index as u64),
            )
            .size(Size::Sm)
            .build(),
        )
        .children(
            (!change.conflicted).then(|| staged_box(change, FocusId(STAGE_BASE + index as u64))),
        )
        .focusable(FocusId(FILE_BASE + index as u64))
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
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(6.5),
            pl(3.0 + depth as f32 * INDENT),
            pr(3),
            flex_none(),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
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
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(&folder.name, 13, 1.4, Role::Text).styles([truncate()])),
        )
        .child(Tally::new(added, removed).size(11).build())
        .focusable(FocusId(FOLDER_BASE + position as u64))
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

/// The rows of the changes of `session`: each group that has files, under its heading, as a tree of folders.
pub fn rows(state: &WorkspaceState, session: &Session) -> Vec<Node> {
    let mut rows = Vec::new();
    let mut counter = 0;
    for group in Group::ALL {
        let files = members(&session.changes, group);
        if files.is_empty() {
            continue;
        }
        rows.push(heading(&session.changes, group));
        rows.extend(rows_of(
            state,
            &session.changes,
            &tree(&session.changes, &files),
            group,
            &files,
            0,
            &mut counter,
        ));
    }
    rows
}

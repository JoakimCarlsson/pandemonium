//! The changed files in groups: staged, tracked and untracked, each file with how it changed, a way to open it and a box that stages it.

use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};

use std::collections::HashSet;

use crate::file_icon::file_icon;
use crate::git_controls::{GROUP_TRACKED, GROUP_UNTRACKED, OPEN_ARROW_BASE};
use crate::kit::{FocusId, IconName, Radius, Role, Side, Styled, Tone, Type, label, label_with};
use crate::parts::glyph;
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
        .filter(|(_, change)| group.holds(change) && change.is_listed_in(repository))
        .map(|(index, _)| index)
        .collect()
}

/// The colour the letter that says how `change` came to differ is written in.
pub(crate) fn mark_color(letter: &str) -> Tone {
    match letter {
        "A" | "U" => Tone::from(Role::Success),
        "D" => Tone::from(Role::Subtle),
        "C" => Tone::from(Role::Danger),
        _ => Tone::rgb(0xe6, 0xc8, 0x4f),
    }
}

/// Emits the box that says how much of a file is in the index.
#[track_caller]
fn staged_box(change: &Change, id: FocusId) {
    Checkbox::new(change.fully_staged(), id)
        .mixed(change.staged && change.unstaged)
        .build();
}

/// Emits the heading above the files of `group` in `repository`, and the box that stages them all: none of them, all of them or some.
#[track_caller]
fn heading(changes: &[Change], group: Group, repository: Option<&str>) {
    let files = members(changes, group, repository);
    let staged = files
        .iter()
        .filter(|&&index| changes[index].fully_staged())
        .count();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .pad_side(Side::Left, 3)
            .pad_side(Side::Right, 3)
            .pad_side(Side::Top, 2)
            .pad_side(Side::Bottom, 1),
        Arrangement::SpaceBetween,
        Alignment::Center,
        || {
            label(
                format!("{} ({})", group.label(), files.len()),
                Type::new(11).ratio(1.4).tone(Role::Subtle),
            );
            if let Some(id) = group.id() {
                Checkbox::new(staged == files.len(), id)
                    .mixed(staged > 0 && staged < files.len())
                    .build();
            }
        },
    );
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

/// The repositories the changes are in when the project holds several side by side, each by its folder name and each once, in the order they came in; nothing inside a submodule, where the changes are all in the one repository.
fn repository_folders<'a>(changes: &'a [Change], repository: Option<&str>) -> Vec<&'a str> {
    let mut found: Vec<&str> = Vec::new();
    if repository.is_none() {
        for name in changes
            .iter()
            .filter_map(|change| change.repository.as_deref())
        {
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found
}

/// The folders of the tree of `repository` whose rows always end at them: submodules moved to another commit and the repositories of a project that holds several.
fn stops<'a>(changes: &'a [Change], repository: Option<&str>) -> Vec<&'a str> {
    let mut found = submodule_folders(changes, repository);
    found.extend(repository_folders(changes, repository));
    found
}

/// The tree of the files of `group` in `repository`: folders that hold only a folder are joined into one row, except that the row of a submodule or of a repository always ends at it.
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
        &stops(changes, repository),
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

/// Emits the row of the file at `index`, `depth` levels down: how it changed, a button that opens it and the box that stages it.
#[track_caller]
fn file_row(state: &WorkspaceState, change: &Change, index: usize, name: &str, depth: usize) {
    let letter = change.mark();
    let tone = mark_color(letter);
    Tile::new(FocusId(FILE_BASE + index as u64))
        .modifier(Modifier::new().fill_max_width())
        .selected(state.git.marked.contains(&index))
        .height(6.5)
        .gap(2.0)
        .radius(Radius::Sm)
        .modifier(Modifier::new().clip_to(Radius::None))
        .inner(
            Modifier::new()
                .pad_side(Side::Left, 3.0 + depth as f32 * INDENT + FILE_OFFSET)
                .pad_side(Side::Right, 3),
        )
        .build_with(|| {
            match &change.moved {
                Some(_) => glyph(IconName::Git, 3.5, Role::Muted),
                None => file_icon(name, 3.5),
            }
            label_with(
                Modifier::new().weight(1.0),
                name,
                Type::new(13).ratio(1.4).tone(Role::Text).truncate(),
            );
            if let Some(moved) = change.moved.as_ref() {
                label(
                    commit_count(moved),
                    Type::new(11).ratio(1.4).tone(Role::Subtle).truncate(),
                );
            }
            label(letter, Type::new(11).ratio(1.4).mono().tone(tone));
            if change.moved.is_none() {
                Button::icon(
                    IconName::ArrowRight,
                    FocusId(OPEN_ARROW_BASE + index as u64),
                )
                .size(Size::Sm)
                .build();
            }
            if !change.conflicted {
                staged_box(change, FocusId(STAGE_BASE + index as u64));
            }
        });
}

/// What the branch of the repository `standing` says stands as is written: its name, or the commit on a detached head, and how many commits it is ahead of and behind what it follows.
fn standing_label(standing: &pm_git::Standing) -> String {
    let mut label = match (&standing.branch, &standing.head) {
        (Some(branch), _) => branch.clone(),
        (None, Some(head)) => format!("@ {head}"),
        (None, None) => String::new(),
    };
    if standing.tracking.ahead > 0 {
        label.push_str(&format!(" ↑{}", standing.tracking.ahead));
    }
    if standing.tracking.behind > 0 {
        label.push_str(&format!(" ↓{}", standing.tracking.behind));
    }
    label
}

/// Emits the row of a folder, `depth` levels down, open or folded: a chevron, the box that stages everything in it, its name and what it changed; a repository's row says what it has checked out, from `standings`.
#[track_caller]
fn folder_row(
    changes: &[Change],
    standings: &[pm_git::Standing],
    folder: &Folder,
    files: &[usize],
    position: usize,
    depth: usize,
    open: bool,
) {
    let inside: Vec<usize> = folder.files.iter().map(|&place| files[place]).collect();
    let staged = inside
        .iter()
        .filter(|&&index| changes[index].fully_staged())
        .count();
    let partly = inside.iter().any(|&index| changes[index].staged);
    let added = inside.iter().map(|&index| changes[index].added()).sum();
    let removed = inside.iter().map(|&index| changes[index].removed()).sum();
    Tile::new(FocusId(FOLDER_BASE + position as u64))
        .modifier(Modifier::new().fill_max_width())
        .height(6.5)
        .gap(2.0)
        .radius(Radius::Sm)
        .inner(
            Modifier::new()
                .pad_side(Side::Left, 3.0 + depth as f32 * INDENT)
                .pad_side(Side::Right, 3),
        )
        .build_with(|| {
            glyph(
                if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                },
                3.5,
                Role::Muted,
            );
            Checkbox::new(
                !inside.is_empty() && staged == inside.len(),
                FocusId(FOLDER_STAGE_BASE + position as u64),
            )
            .mixed(partly && staged < inside.len())
            .build();
            if stops(changes, None).contains(&folder.path.as_str()) {
                glyph(IconName::Git, 3.5, Role::Muted);
            }
            label_with(
                Modifier::new().weight(1.0),
                &folder.name,
                Type::new(13).ratio(1.4).tone(Role::Text).truncate(),
            );
            if let Some(standing) = standings
                .iter()
                .find(|standing| standing.path == folder.path)
            {
                label(
                    standing_label(standing),
                    Type::new(11)
                        .ratio(1.4)
                        .tone(Role::Subtle)
                        .truncate()
                        .mono(),
                );
            }
            Tally::new(added, removed).size(11).build();
        });
}

/// Emits the rows of `entries` and of everything in their open folders, `depth` levels down; `counter` numbers the folders across the groups as [`folder_table`] does.
fn rows_of(
    state: &WorkspaceState,
    session: &Session,
    entries: &[Entry],
    group: Group,
    files: &[usize],
    depth: usize,
    counter: &mut usize,
) {
    for entry in entries {
        match entry {
            Entry::File { name, index } => {
                let at = files[*index];
                rustle::key(at, || {
                    file_row(state, &session.changes[at], at, name, depth)
                });
            }
            Entry::Folder(folder) => {
                let position = *counter;
                *counter += 1;
                let open = !state
                    .closed_folders
                    .contains(&fold_key(group, &folder.path));
                rustle::key(("folder", position), || {
                    folder_row(
                        &session.changes,
                        &session.standings,
                        folder,
                        files,
                        position,
                        depth,
                        open,
                    )
                });
                if open {
                    rows_of(
                        state,
                        session,
                        &folder.entries,
                        group,
                        files,
                        depth + 1,
                        counter,
                    );
                } else {
                    *counter += folders(&folder.entries).len();
                }
            }
        }
    }
}

/// Emits the rows of the changes of `session` in the repository the panel works on: each group that has files, under its heading, as a tree of folders.
#[track_caller]
pub fn rows(state: &WorkspaceState, session: &Session) {
    let repository = state.git.repository.as_deref();
    let mut counter = 0;
    for group in Group::ALL {
        let files = members(&session.changes, group, repository);
        if files.is_empty() {
            continue;
        }
        heading(&session.changes, group, repository);
        rows_of(
            state,
            session,
            &tree(&session.changes, &files, repository),
            group,
            &files,
            0,
            &mut counter,
        );
    }
}

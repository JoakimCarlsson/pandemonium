//! The menu on the tab of an open file: closing it and the tabs around it, making it read only, its paths and permalink, showing it in the file manager and in the project's files, pinning it, and a shell beside it.

use pm_input::FocusId;
use pm_ui::Node;

use crate::Menu;
use crate::open_files::{Contents, Viewer};
use crate::state::{Outgoing, ReviewTab, WorkspaceState};

/// The first of the rows of the menu on a tab; each command's row follows it by the command's place.
const TAB_MENU_BASE: u64 = 7_403_000;

/// How tall the menu on a tab is, in logical pixels, to keep it inside the window.
pub const TAB_MENU_HEIGHT: f32 = 15.0 * 26.0 + 7.0 * 9.0 + 10.0;

/// What a row of the menu on a tab does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabCommand {
    /// Closes the tab.
    Close,
    /// Closes every other tab that is not pinned.
    CloseOthers,
    /// Closes the tabs left of it that are not pinned.
    CloseLeft,
    /// Closes the tabs right of it that are not pinned.
    CloseRight,
    /// Closes the tabs without changes to save that are not pinned.
    CloseClean,
    /// Closes every tab that is not pinned.
    CloseAll,
    /// Makes the file read only, or editable again.
    ToggleReadOnly,
    /// Copies the path of the file on its machine.
    CopyPath,
    /// Copies the path of the file from the root of the project.
    CopyRelativePath,
    /// Opens the address of the file on the host of its repository.
    OpenPermalink,
    /// Copies the address of the file on the host of its repository.
    CopyPermalink,
    /// Shows the file in the file manager of this machine.
    Reveal,
    /// Pins the tab, or unpins it.
    TogglePin,
    /// Shows the file among the project's files.
    RevealInProject,
    /// Starts a shell in the folder of the file.
    OpenInTerminal,
}

impl TabCommand {
    /// Every command, in the order of their rows' names.
    const ALL: [Self; 15] = [
        Self::Close,
        Self::CloseOthers,
        Self::CloseLeft,
        Self::CloseRight,
        Self::CloseClean,
        Self::CloseAll,
        Self::ToggleReadOnly,
        Self::CopyPath,
        Self::CopyRelativePath,
        Self::OpenPermalink,
        Self::CopyPermalink,
        Self::Reveal,
        Self::TogglePin,
        Self::RevealInProject,
        Self::OpenInTerminal,
    ];

    /// The name of the command's row.
    pub fn id(self) -> FocusId {
        let place = Self::ALL
            .iter()
            .position(|held| *held == self)
            .unwrap_or_default();
        FocusId(TAB_MENU_BASE + place as u64)
    }

    /// The command whose row `id` names.
    pub fn of(id: FocusId) -> Option<Self> {
        let place = id.0.checked_sub(TAB_MENU_BASE)?;
        Self::ALL.get(usize::try_from(place).ok()?).copied()
    }
}

/// The menu on a tab, open over the tab at `index`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabMenu {
    /// The tab, by position.
    pub index: usize,
    /// Where the menu opened, in logical pixels.
    pub at: (f32, f32),
}

/// Whether the text of `viewer` can be changed.
fn editable(viewer: &Viewer) -> bool {
    matches!(&viewer.contents, Contents::Text(document) if !document.field.is_read_only())
}

impl WorkspaceState {
    /// The state with the menu on the tab at `index` open at `at`.
    pub fn with_tab_menu(mut self, index: usize, at: (f32, f32)) -> Self {
        if index < self.git.viewers.len() {
            self.tab_menu = Some(TabMenu { index, at });
        }
        self
    }

    /// Whether `id` is a row of the menu on a tab while it is open.
    pub fn is_tab_menu_row(&self, id: FocusId) -> bool {
        self.tab_menu.is_some() && TabCommand::of(id).is_some()
    }

    /// The folder of the session the review panel is about on its machine.
    fn session_root(&self) -> Option<String> {
        let session = self.review_session()?;
        if !session.directory.is_empty() {
            return Some(session.directory.clone());
        }
        let project = self.projects.get(session.project)?;
        Some(project.directory().to_string_lossy().into_owned())
    }

    /// Closes the tabs at `indexes` that are not pinned, those with nothing to save at once, and asks about the first that has changes.
    fn close_tabs(&mut self, indexes: Vec<usize>) {
        let mut asking = None;
        for index in indexes.into_iter().rev() {
            let Some(viewer) = self.git.viewers.get(index) else {
                continue;
            };
            if viewer.pinned {
                continue;
            }
            if viewer.is_dirty() {
                asking = Some(index);
            } else {
                self.discard_file(index);
                asking = asking.map(|held: usize| if held > index { held - 1 } else { held });
            }
        }
        if let Some(index) = asking {
            self.close_file(index);
        }
    }

    /// Carries out `command` on the tab the menu is open over, and closes the menu.
    pub fn tab_command(mut self, command: TabCommand) -> Self {
        let Some(menu) = self.tab_menu.take() else {
            return self;
        };
        let index = menu.index;
        let Some(viewer) = self.git.viewers.get(index) else {
            return self;
        };
        let path = viewer.path.clone();
        let head = viewer.head;
        let count = self.git.viewers.len();
        let session = self.review_index();
        match command {
            TabCommand::Close => {
                if let Some(viewer) = self.git.viewers.get_mut(index) {
                    viewer.pinned = false;
                }
                self.close_tabs(vec![index]);
            }
            TabCommand::CloseOthers => {
                self.close_tabs((0..count).filter(|other| *other != index).collect());
            }
            TabCommand::CloseLeft => self.close_tabs((0..index).collect()),
            TabCommand::CloseRight => self.close_tabs((index + 1..count).collect()),
            TabCommand::CloseClean => {
                let clean = (0..count)
                    .filter(|other| !self.git.viewers[*other].is_dirty())
                    .collect();
                self.close_tabs(clean);
            }
            TabCommand::CloseAll => self.close_tabs((0..count).collect()),
            TabCommand::ToggleReadOnly => {
                if let Some(viewer) = self.git.viewers.get_mut(index)
                    && !head
                    && let Contents::Text(document) = &mut viewer.contents
                    && !document.truncated
                {
                    let read_only = !document.field.is_read_only();
                    document.field = std::mem::take(&mut document.field).read_only(read_only);
                }
            }
            TabCommand::CopyPath => {
                if let Some(root) = self.session_root() {
                    let text = format!("{}/{path}", root.trim_end_matches(['/', '\\']));
                    self.outbox.push(Outgoing::Copy { text });
                }
            }
            TabCommand::CopyRelativePath => self.outbox.push(Outgoing::Copy { text: path }),
            TabCommand::OpenPermalink | TabCommand::CopyPermalink => {
                if let Some(session) = session.filter(|_| !head) {
                    self.outbox.push(Outgoing::Permalink {
                        session,
                        path,
                        line: None,
                        end_line: None,
                        open: command == TabCommand::OpenPermalink,
                    });
                }
            }
            TabCommand::Reveal => {
                if let Some(session) = session {
                    self.outbox.push(Outgoing::RevealFile { session, path });
                }
            }
            TabCommand::TogglePin => self.toggle_pin(index),
            TabCommand::RevealInProject => self.reveal_in_project(&path),
            TabCommand::OpenInTerminal => {
                if let Some(session) = session {
                    let folder = path
                        .rsplit_once('/')
                        .map(|(folder, _)| folder.to_owned())
                        .unwrap_or_default();
                    self.open_terminal_in(session, folder);
                }
            }
        }
        self
    }

    /// Pins the tab at `index` among the pinned tabs at the start of the strip, or unpins it and puts it after them.
    fn toggle_pin(&mut self, index: usize) {
        if index >= self.git.viewers.len() {
            return;
        }
        let shown = self
            .git
            .shown
            .map(|held| self.git.viewers[held].path.clone());
        let mut viewer = self.git.viewers.remove(index);
        viewer.pinned = !viewer.pinned;
        viewer.preview = false;
        let pinned = self.git.viewers.iter().filter(|held| held.pinned).count();
        self.git.viewers.insert(pinned, viewer);
        self.git.shown =
            shown.and_then(|path| self.git.viewers.iter().position(|held| held.path == path));
    }

    /// Shows the file at `path` among the project's files: the panel on its files, the folders down to it open and the file picked.
    fn reveal_in_project(&mut self, path: &str) {
        self.show_review_tab(ReviewTab::Files);
        let mut folder = String::new();
        for part in path
            .split('/')
            .collect::<Vec<_>>()
            .split_last()
            .map_or(&[][..], |(_, rest)| rest)
        {
            if !folder.is_empty() {
                folder.push('/');
            }
            folder.push_str(part);
            self.open_tree.insert(folder.clone());
        }
        self.git.tree_selected = Some(crate::tree_actions::TreeTarget {
            path: path.to_owned(),
            folder: false,
        });
    }
}

/// The menu on the tab of `viewer`, at `at`, its rows enabled by what the file allows, `local` when the project is on this machine.
pub fn tab_menu(viewer: &Viewer, open: TabMenu, count: usize, local: bool) -> Node {
    let row = |menu: Menu, label: &str, keys: Option<&str>, command: TabCommand, on: bool| {
        menu.item(label, keys, command.id(), !on)
    };
    let in_folder = !viewer.head;
    let read_only = !editable(viewer);
    let menu = Menu::new(open.at.0, open.at.1).width(62.0).row_height(6.5);
    let menu = row(menu, "Close", None, TabCommand::Close, true);
    let menu = row(
        menu,
        "Close Others",
        None,
        TabCommand::CloseOthers,
        count > 1,
    )
    .separator();
    let menu = row(
        menu,
        "Close Left",
        None,
        TabCommand::CloseLeft,
        open.index > 0,
    );
    let menu = row(
        menu,
        "Close Right",
        None,
        TabCommand::CloseRight,
        open.index + 1 < count,
    )
    .separator();
    let menu = row(menu, "Close Clean", None, TabCommand::CloseClean, true);
    let menu = row(menu, "Close All", None, TabCommand::CloseAll, true).separator();
    let menu = row(
        menu,
        if read_only {
            "Make Tab Editable"
        } else {
            "Make Tab Read-Only"
        },
        None,
        TabCommand::ToggleReadOnly,
        in_folder,
    )
    .separator();
    let menu = row(menu, "Copy Path", None, TabCommand::CopyPath, in_folder);
    let menu = row(
        menu,
        "Copy Relative Path",
        None,
        TabCommand::CopyRelativePath,
        true,
    )
    .separator();
    let menu = row(
        menu,
        "Open File Permalink",
        None,
        TabCommand::OpenPermalink,
        in_folder,
    );
    let menu = row(
        menu,
        "Copy File Permalink",
        None,
        TabCommand::CopyPermalink,
        in_folder,
    )
    .separator();
    let menu = row(
        menu,
        "Reveal in File Manager",
        None,
        TabCommand::Reveal,
        local && in_folder,
    )
    .separator();
    let menu = row(
        menu,
        if viewer.pinned {
            "Unpin Tab"
        } else {
            "Pin Tab"
        },
        None,
        TabCommand::TogglePin,
        true,
    );
    let menu = row(
        menu,
        "Reveal in Project Panel",
        None,
        TabCommand::RevealInProject,
        in_folder,
    );
    row(
        menu,
        "Open in Terminal",
        None,
        TabCommand::OpenInTerminal,
        in_folder,
    )
    .build()
}

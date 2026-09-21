//! The shells the window has running, listed per project.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use pm_core::ProjectId;
use pm_vt::{Notify, Terminal};

/// Columns a shell is started with, before a pane has been drawn for it.
const INITIAL_COLS: usize = 80;

/// Rows a shell is started with, before a pane has been drawn for it.
const INITIAL_ROWS: usize = 24;

/// One running shell, shared between the window and the pane drawing it.
///
/// The element tree is rebuilt every frame and may not borrow the window's
/// state, so the pane holds the shell itself rather than a reference to where
/// the window keeps it.
pub type Shell = Rc<RefCell<Terminal>>;

/// A shell's identity for as long as it is running.
///
/// Ids are handed out by [`Terminals`] and are unique across the window, so a
/// list row, a keybinding and a pane all name the same shell without knowing
/// which project it belongs to.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShellId(u64);

/// One shell as the list of them presents it.
pub struct ShellEntry {
    /// Which shell this row is.
    pub id: ShellId,
    /// What the row calls it: the program, or the title the program set.
    pub name: String,
    /// Whether this is the shell the pane is showing.
    pub active: bool,
}

/// One project's shells and which of them its pane is showing.
#[derive(Default)]
struct ProjectShells {
    /// The shells, in the order they were started.
    running: Vec<(ShellId, Shell)>,
    /// The one the pane is showing.
    active: Option<ShellId>,
}

impl ProjectShells {
    /// The shell `id` names, if it is still running.
    fn get(&self, id: ShellId) -> Option<Shell> {
        self.running
            .iter()
            .find(|(running, _)| *running == id)
            .map(|(_, shell)| shell.clone())
    }

    /// The shell the pane is showing.
    fn active(&self) -> Option<Shell> {
        self.active.and_then(|id| self.get(id))
    }

    /// Drops the shells whose child has exited, and says whether any had.
    fn reap(&mut self) -> bool {
        let before = self.running.len();
        self.running
            .retain(|(_, shell)| shell.borrow_mut().is_running());
        if self.running.len() == before {
            return false;
        }
        if self.active().is_none() {
            self.active = self.running.last().map(|(id, _)| *id);
        }
        true
    }
}

/// Every shell the window is running, and which one each project is showing.
///
/// A project's first shell is started the first time its pane is drawn, and
/// the rest when they are asked for; they live until they exit or the project
/// leaves the window. Switching projects switches lists rather than
/// restarting anything.
#[derive(Default)]
pub struct Terminals {
    /// The shells, by the project whose worktree they are running in.
    projects: BTreeMap<ProjectId, ProjectShells>,
    /// The id the next shell started will be given.
    next: ShellId,
    /// What a shell calls when it has written something.
    notify: Option<Notify>,
}

impl Terminals {
    /// Wakes the window through `notify` whenever a shell writes something.
    pub fn set_notify(&mut self, notify: Notify) {
        self.notify = Some(notify);
    }

    /// The shell `project` is showing, starting its first one in `root`.
    pub fn open(&mut self, project: ProjectId, root: &Path) -> Option<Shell> {
        if self
            .projects
            .get(&project)
            .is_none_or(|shells| shells.running.is_empty())
        {
            self.start(project, root);
        }
        self.active(project)
    }

    /// Starts another shell in `root` and shows it.
    pub fn start(&mut self, project: ProjectId, root: &Path) -> Option<ShellId> {
        let notify = self.notify.clone()?;
        let shell = match Terminal::shell(root, INITIAL_COLS, INITIAL_ROWS, notify) {
            Ok(shell) => shell,
            Err(error) => {
                eprintln!("could not start a shell in {}: {error}", root.display());
                return None;
            }
        };

        let id = self.next;
        self.next = ShellId(id.0 + 1);
        let shells = self.projects.entry(project).or_default();
        shells.running.push((id, Rc::new(RefCell::new(shell))));
        shells.active = Some(id);
        Some(id)
    }

    /// The shell `project` is showing, if it has one.
    pub fn active(&self, project: ProjectId) -> Option<Shell> {
        self.projects.get(&project)?.active()
    }

    /// Shows the shell `id` names.
    pub fn activate(&mut self, project: ProjectId, id: ShellId) {
        if let Some(shells) = self.projects.get_mut(&project)
            && shells.get(id).is_some()
        {
            shells.active = Some(id);
        }
    }

    /// Every shell of `project`, in the order they were started.
    pub fn list(&self, project: ProjectId) -> Vec<ShellEntry> {
        let Some(shells) = self.projects.get(&project) else {
            return Vec::new();
        };
        shells
            .running
            .iter()
            .map(|(id, shell)| ShellEntry {
                id: *id,
                name: name(shell),
                active: shells.active == Some(*id),
            })
            .collect()
    }

    /// How many shells `project` has running.
    pub fn count(&self, project: ProjectId) -> usize {
        self.projects
            .get(&project)
            .map_or(0, |shells| shells.running.len())
    }

    /// Stops the shell `id` names, showing another of the project's instead.
    pub fn stop(&mut self, project: ProjectId, id: ShellId) {
        let Some(shells) = self.projects.get_mut(&project) else {
            return;
        };
        shells.running.retain(|(running, _)| *running != id);
        if shells.active == Some(id) {
            shells.active = shells.running.last().map(|(id, _)| *id);
        }
    }

    /// Stops every shell of `project` but the one `id` names.
    pub fn stop_others(&mut self, project: ProjectId, id: ShellId) {
        let Some(shells) = self.projects.get_mut(&project) else {
            return;
        };
        shells.running.retain(|(running, _)| *running == id);
        shells.active = Some(id);
    }

    /// Stops every shell of `project`, leaving the project open.
    pub fn stop_all(&mut self, project: ProjectId) {
        if let Some(shells) = self.projects.get_mut(&project) {
            shells.running.clear();
            shells.active = None;
        }
    }

    /// Stops every shell of `project`, for a project leaving the window.
    pub fn close(&mut self, project: ProjectId) {
        self.projects.remove(&project);
    }

    /// Applies what every shell has written, and says whether anything changed.
    ///
    /// A shell whose child has exited is dropped here rather than left in the
    /// list as a dead pane: the list shows what is running.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        for shells in self.projects.values_mut() {
            for (_, shell) in &shells.running {
                changed |= shell.borrow_mut().pump();
            }
            changed |= shells.reap();
        }
        changed
    }
}

/// What a tab calls `shell`: the program it runs, as `bash` or `zsh`.
///
/// The title a shell sets is its whole prompt, which is far too long to name
/// a tab with and says the same thing on every one of them. The prompt
/// belongs in the pane, where it is already drawn.
fn name(shell: &Shell) -> String {
    let shell = shell.borrow();
    let program = shell.program();
    Path::new(program).file_name().map_or_else(
        || program.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

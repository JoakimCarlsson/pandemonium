//! The shells the window has running, listed per worktree.

use pm_host::Location;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pm_core::{ProjectId, Scope, Task};
use pm_vt::{Notify, Terminal};
use serde::{Deserialize, Serialize};

/// Columns a shell is started with, before a pane has been drawn for it.
const INITIAL_COLS: usize = 80;

/// Rows a shell is started with, before a pane has been drawn for it.
const INITIAL_ROWS: usize = 24;

/// Columns a command an agent runs is started with.
///
/// Nobody may ever draw it, and what it writes is read back as text, so it
/// is given room for the lines a build or a test run prints rather than the
/// width of a pane that does not exist yet.
const ERRAND_COLS: usize = 160;

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
/// which worktree it belongs to.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShellId(u64);

/// One shell as the list of them presents it.
pub struct ShellEntry {
    /// Which shell this row is.
    pub id: ShellId,
    /// What the row calls it: the name the reader gave it, or the program.
    pub name: String,
    /// Whether this is the shell the pane is showing.
    pub active: bool,
}

/// One shell as it is written down, for the next launch to start again.
///
/// A shell names its worktree by where it lives rather than by the scope this
/// run gave it, because a scope means nothing to the next launch.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SavedShell {
    /// The root of the worktree it ran in.
    pub worktree: PathBuf,
    /// The name the reader gave it, empty when it goes by its program.
    pub name: String,
    /// Whether it was the shell its worktree's pane was showing.
    pub active: bool,
}

/// A shell that exited with something other than success.
pub struct Exited {
    /// The worktree it was running in.
    pub scope: Scope,
    /// What it was called.
    pub name: String,
    /// The code it exited with.
    pub code: u32,
}

/// A shell an agent started to run one command.
struct Errand {
    /// What the list calls it: the command it runs.
    label: String,
    /// Whether the agent may still ask about it, which keeps it listed after
    /// its command has exited so that what it wrote can still be read.
    held: bool,
}

/// One worktree's shells and which of them its pane is showing.
#[derive(Default)]
struct WorktreeShells {
    /// The shells, in the order they were started.
    running: Vec<(ShellId, Shell)>,
    /// The one the pane is showing.
    active: Option<ShellId>,
    /// The shells among them that agents started, by id.
    errands: BTreeMap<ShellId, Errand>,
    /// Task shells, retained after their children exit.
    tasks: BTreeMap<ShellId, String>,
    /// The names the reader gave shells, by id.
    names: BTreeMap<ShellId, String>,
}

impl WorktreeShells {
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

    /// What the list calls the plain shell `id` names: the name the reader
    /// gave it, or else the program it runs.
    fn label(&self, id: ShellId, shell: &Shell) -> String {
        self.names.get(&id).cloned().unwrap_or_else(|| name(shell))
    }

    /// Drops reader shells whose child has exited, and says whether any had.
    ///
    /// The ones that exited badly are handed to `failed`, named, with the
    /// code they gave.
    ///
    /// A shell an agent still holds is kept however it ended, and one an
    /// agent started is never reported: a failing test is the agent's to
    /// read, not the reader's to be interrupted by. Task shells remain
    /// listed and report through the task seam.
    fn reap(&mut self, scope: Scope, failed: &mut Vec<Exited>) -> bool {
        let before = self.running.len();
        let errands = &mut self.errands;
        let names = &mut self.names;
        self.running.retain(|(id, shell)| {
            let mut child = shell.borrow_mut();
            if child.is_running() {
                return true;
            }
            if let Some(errand) = errands.get(id) {
                if errand.held {
                    return true;
                }
                errands.remove(id);
                return false;
            }
            if self.tasks.contains_key(id) {
                return true;
            }
            let given = names.remove(id);
            if let Some(code) = child.exit_code().filter(|code| *code != 0) {
                drop(child);
                failed.push(Exited {
                    scope,
                    name: given.unwrap_or_else(|| name(shell)),
                    code,
                });
            }
            false
        });
        if self.running.len() == before {
            return false;
        }
        if self.active().is_none() {
            self.active = self.running.last().map(|(id, _)| *id);
        }
        true
    }
}

/// Every shell the window is running, and which one each worktree is showing.
///
/// A worktree's first shell is started the first time its pane is drawn, and
/// the rest when they are asked for; they live until they exit or the project
/// leaves the window. Switching worktree switches lists rather than
/// restarting anything: a session's shells run in the session's own
/// directory, which is the whole point of its having one.
#[derive(Default)]
pub struct Terminals {
    /// The shells, by the worktree they are running in.
    worktrees: BTreeMap<Scope, WorktreeShells>,
    /// The id the next shell started will be given.
    next: ShellId,
    /// What a shell calls when it has written something.
    notify: Option<Notify>,
    /// How many lines of scrollback a shell keeps, once the reader has said.
    scrollback: Option<usize>,
    /// The shells that exited badly since this was last asked.
    failed: Vec<Exited>,
}

impl Terminals {
    /// Wakes the window through `notify` whenever a shell writes something.
    pub fn set_notify(&mut self, notify: Notify) {
        self.notify = Some(notify);
    }

    /// Keeps `lines` of scrollback in every shell from now on, the running
    /// ones included.
    pub fn set_scrollback(&mut self, lines: usize) {
        if self.scrollback == Some(lines) {
            return;
        }
        self.scrollback = Some(lines);
        for (_, shell) in self.worktrees.values().flat_map(|shells| &shells.running) {
            shell.borrow_mut().set_scrollback(lines);
        }
    }

    /// The shell `scope` is showing, starting its first one in `root`.
    ///
    /// The commands agents are running are not the reader's shell: a
    /// worktree with only those has its own started beside them.
    pub fn open(
        &mut self,
        scope: Scope,
        root: &Location,
        env: &[(String, String)],
    ) -> Option<Shell> {
        if self.worktrees.get(&scope).is_none_or(|shells| {
            shells
                .running
                .iter()
                .all(|(id, _)| shells.errands.contains_key(id) || shells.tasks.contains_key(id))
        }) {
            let active_task = self.active_task(scope);
            self.start(scope, root, env);
            if let Some(task) = active_task {
                self.activate(scope, task);
            }
        }
        self.active(scope)
    }

    /// Starts another shell in `root` and shows it.
    ///
    /// The `env` is the worktree's own, so a shell opened in a session serves
    /// on the session's port rather than on whatever the last one took.
    pub fn start(
        &mut self,
        scope: Scope,
        root: &Location,
        env: &[(String, String)],
    ) -> Option<ShellId> {
        let notify = self.notify.clone()?;
        let mut shell = match Terminal::shell(root, INITIAL_COLS, INITIAL_ROWS, env, notify) {
            Ok(shell) => shell,
            Err(error) => {
                eprintln!("could not start a shell in {}: {error}", root.display());
                return None;
            }
        };

        if let Some(lines) = self.scrollback {
            shell.set_scrollback(lines);
        }
        let id = self.next;
        self.next = ShellId(id.0 + 1);
        let shells = self.worktrees.entry(scope).or_default();
        shells.running.push((id, Rc::new(RefCell::new(shell))));
        shells.active = Some(id);
        Some(id)
    }

    /// Runs `program` with `args` in `cwd` for an agent, as a shell of
    /// `scope`'s that lists as `label`.
    ///
    /// It joins the worktree's list without taking the pane from the shell
    /// the reader is looking at, and it stays in the list until the agent
    /// lets it go, however soon its command exits.
    pub fn run(
        &mut self,
        scope: Scope,
        cwd: &Location,
        program: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> std::io::Result<ShellId> {
        let notify = self
            .notify
            .clone()
            .ok_or_else(|| std::io::Error::other("the window is not listening"))?;
        let mut shell = Terminal::run(cwd, ERRAND_COLS, INITIAL_ROWS, program, args, env, notify)?;
        if let Some(lines) = self.scrollback {
            shell.set_scrollback(lines);
        }
        let id = self.next;
        self.next = ShellId(id.0 + 1);
        let shells = self.worktrees.entry(scope).or_default();
        shells.running.push((id, Rc::new(RefCell::new(shell))));
        shells.active = shells.active.or(Some(id));
        let label = std::iter::once(program)
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        shells.errands.insert(id, Errand { label, held: true });
        Ok(id)
    }

    /// Starts a retained task shell, optionally replacing an earlier run in its list position.
    pub fn run_task(
        &mut self,
        scope: Scope,
        cwd: &Location,
        task: &Task,
        env: &[(String, String)],
        replace: Option<ShellId>,
        front: bool,
    ) -> std::io::Result<ShellId> {
        let notify = self
            .notify
            .clone()
            .ok_or_else(|| std::io::Error::other("the window is not listening"))?;
        let mut shell = Terminal::run(
            cwd,
            INITIAL_COLS,
            INITIAL_ROWS,
            &task.command,
            &[],
            env,
            notify,
        )?;
        if let Some(lines) = self.scrollback {
            shell.set_scrollback(lines);
        }
        let id = self.next;
        self.next = ShellId(id.0 + 1);
        let shells = self.worktrees.entry(scope).or_default();
        let entry = (id, Rc::new(RefCell::new(shell)));
        if let Some(position) =
            replace.and_then(|old| shells.running.iter().position(|(id, _)| *id == old))
        {
            shells.running[position] = entry;
            shells.tasks.remove(&replace.unwrap());
        } else {
            shells.running.push(entry);
        }
        shells.tasks.insert(id, task.label.clone());
        if front {
            shells.active = Some(id);
        } else {
            shells.active = shells.active.or(Some(id));
        }
        Ok(id)
    }

    /// The task shell `id` names if it is still in the worktree's list.
    pub fn task_shell(&self, scope: Scope, id: ShellId) -> Option<Shell> {
        let shells = self.worktrees.get(&scope)?;
        shells
            .tasks
            .contains_key(&id)
            .then(|| shells.get(id))
            .flatten()
    }

    /// The task shell currently shown in the terminal view.
    pub fn active_task(&self, scope: Scope) -> Option<ShellId> {
        let shells = self.worktrees.get(&scope)?;
        shells.active.filter(|id| shells.tasks.contains_key(id))
    }

    /// The shell `id` names in `scope`, while it is listed.
    pub fn get(&self, scope: Scope, id: ShellId) -> Option<Shell> {
        self.worktrees.get(&scope)?.get(id)
    }

    /// Lets go of a shell an agent started, which leaves the list once its
    /// command has exited.
    pub fn release(&mut self, scope: Scope, id: ShellId) {
        if let Some(errand) = self
            .worktrees
            .get_mut(&scope)
            .and_then(|shells| shells.errands.get_mut(&id))
        {
            errand.held = false;
        }
    }

    /// The id of the shell `scope` is showing, if it has one.
    pub fn active_id(&self, scope: Scope) -> Option<ShellId> {
        let shells = self.worktrees.get(&scope)?;
        shells.active.filter(|id| shells.get(*id).is_some())
    }

    /// Calls the shell `id` names `name` from now on, or by its program again
    /// when `name` is empty once trimmed.
    ///
    /// Shells an agent or a task started keep the label they were started with.
    pub fn rename(&mut self, scope: Scope, id: ShellId, name: &str) {
        let Some(shells) = self.worktrees.get_mut(&scope) else {
            return;
        };
        if shells.get(id).is_none()
            || shells.errands.contains_key(&id)
            || shells.tasks.contains_key(&id)
        {
            return;
        }
        match name.trim() {
            "" => shells.names.remove(&id),
            name => shells.names.insert(id, name.to_owned()),
        };
    }

    /// The shell `scope` is showing, if it has one.
    pub fn active(&self, scope: Scope) -> Option<Shell> {
        self.worktrees.get(&scope)?.active()
    }

    /// Shows the shell `id` names.
    pub fn activate(&mut self, scope: Scope, id: ShellId) {
        if let Some(shells) = self.worktrees.get_mut(&scope)
            && shells.get(id).is_some()
        {
            shells.active = Some(id);
        }
    }

    /// Every shell of `scope`, in the order they were started.
    pub fn list(&self, scope: Scope) -> Vec<ShellEntry> {
        let Some(shells) = self.worktrees.get(&scope) else {
            return Vec::new();
        };
        shells
            .running
            .iter()
            .map(|(id, shell)| ShellEntry {
                id: *id,
                name: shells.errands.get(id).map_or_else(
                    || {
                        shells.tasks.get(id).map_or_else(
                            || shells.label(*id, shell),
                            |label| {
                                let mut child = shell.borrow_mut();
                                if child.is_running() {
                                    label.clone()
                                } else if child.exit_code() == Some(0) {
                                    format!("✓ {label}")
                                } else {
                                    format!("✗ {label}")
                                }
                            },
                        )
                    },
                    |errand| errand.label.clone(),
                ),
                active: shells.active == Some(*id),
            })
            .collect()
    }

    /// Every plain shell of the worktrees in `roots`, as the next launch will
    /// start them again.
    ///
    /// Shells an agent or a task started are left out: they belong to the
    /// command that made them.
    pub fn saved(&self, roots: &[(Scope, Location)]) -> Vec<SavedShell> {
        roots
            .iter()
            .filter_map(|(scope, root)| Some((self.worktrees.get(scope)?, root)))
            .flat_map(|(shells, root)| {
                shells
                    .running
                    .iter()
                    .filter(|(id, _)| {
                        !shells.errands.contains_key(id) && !shells.tasks.contains_key(id)
                    })
                    .map(|(id, _)| SavedShell {
                        worktree: root.stored(),
                        name: shells.names.get(id).cloned().unwrap_or_default(),
                        active: shells.active == Some(*id),
                    })
            })
            .collect()
    }

    /// How many shells `scope` has running.
    pub fn count(&self, scope: Scope) -> usize {
        self.worktrees
            .get(&scope)
            .map_or(0, |shells| shells.running.len())
    }

    /// Stops the shell `id` names, showing another of the worktree's instead.
    pub fn stop(&mut self, scope: Scope, id: ShellId) {
        let Some(shells) = self.worktrees.get_mut(&scope) else {
            return;
        };
        shells.running.retain(|(running, _)| *running != id);
        shells.errands.remove(&id);
        shells.tasks.remove(&id);
        shells.names.remove(&id);
        if shells.active == Some(id) {
            shells.active = shells.running.last().map(|(id, _)| *id);
        }
    }

    /// Stops every shell of `scope` but the one `id` names.
    pub fn stop_others(&mut self, scope: Scope, id: ShellId) {
        let Some(shells) = self.worktrees.get_mut(&scope) else {
            return;
        };
        shells.running.retain(|(running, _)| *running == id);
        shells.errands.retain(|errand, _| *errand == id);
        shells.tasks.retain(|task, _| *task == id);
        shells.names.retain(|named, _| *named == id);
        shells.active = Some(id);
    }

    /// Stops every shell of `scope`, leaving the worktree open.
    pub fn stop_all(&mut self, scope: Scope) {
        if let Some(shells) = self.worktrees.get_mut(&scope) {
            shells.running.clear();
            shells.errands.clear();
            shells.tasks.clear();
            shells.names.clear();
            shells.active = None;
        }
    }

    /// Stops every shell of `project`, for a project leaving the window.
    pub fn close(&mut self, project: ProjectId) {
        self.worktrees.retain(|scope, _| scope.project() != project);
    }

    /// Applies what every shell has written, and says whether anything changed.
    ///
    /// Reader shells whose children exited leave the list; task shells stay
    /// so their output can be read after a run.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        for (scope, shells) in &mut self.worktrees {
            for (_, shell) in &shells.running {
                changed |= shell.borrow_mut().pump();
            }
            changed |= shells.reap(*scope, &mut self.failed);
        }
        changed
    }

    /// The shells that exited badly since this was last asked.
    pub fn take_failed(&mut self) -> Vec<Exited> {
        std::mem::take(&mut self.failed)
    }
}

/// What a tab calls `shell` until the reader names it: the program it runs,
/// as `bash` or `zsh`.
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

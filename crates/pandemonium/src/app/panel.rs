//! Worktree tool commands and the diagnostics and debugger views they draw.

use pm_ui::{Div, Theme};

use crate::app::places::Place;
use crate::app::{App, Writing};
use crate::message::Message;
use crate::panel::{PanelView, Problem, ProblemFile};
use crate::panes::Tool;

impl App {
    /// Brings the registered tool for `view` forward in its pane.
    ///
    /// The keyboard goes to the shell when the view is the terminal's, and
    /// is left where it was otherwise.
    pub(super) fn show_panel(&mut self, view: PanelView) {
        self.show_tool(Tool::from(view));
    }

    /// Whether a pane is showing the worktree's shells.
    pub(super) fn showing_terminals(&self) -> bool {
        self.tool_visible(Tool::Terminal)
    }

    /// Starts again the shells the last launch had running, in the worktrees
    /// that are still open, under the names they were given.
    pub(super) fn restore_shells(&mut self, saved: &[crate::terminal::SavedShell]) {
        let worktrees = self.worktrees();
        for shell in saved {
            let Some((scope, root)) = worktrees
                .iter()
                .find(|(_, root)| root.stored() == shell.worktree)
            else {
                continue;
            };
            let env = self.worktree_env(*scope);
            let Some(id) = self.terminals.start(*scope, root, &env) else {
                continue;
            };
            self.terminals.rename(*scope, id, &shell.name);
            if !shell.active {
                continue;
            }
            self.terminals.activate(*scope, id);
        }
    }

    /// Whether a pane is showing the debugger.
    pub(super) fn showing_debugger(&self) -> bool {
        self.tool_visible(Tool::Debug)
    }

    /// Carries out commands sent by worktree tool views.
    pub(super) fn panel_command(&mut self, message: Message) -> bool {
        match message {
            Message::ShowPanelView(view) => self.show_panel(view),
            Message::TogglePanelView(view) => {
                let tool = Tool::from(view);
                match self.tool_pane(tool) {
                    Some(pane) if self.tool_visible(tool) => {
                        self.close_item(pane, self.tool_item(tool))
                    }
                    _ => self.show_tool(tool),
                }
            }
            Message::OpenProblem(file, position) => {
                if let (Some(scope), Some(path)) =
                    (self.editor.scope_of(file), self.editor.path(file))
                {
                    self.terminal_focused = false;
                    self.jump_to(&Place {
                        scope,
                        path,
                        position,
                    });
                }
            }
            _ => return false,
        }
        true
    }

    /// Every open file of the worktree in front that a server has something
    /// to say about, and what it says.
    pub(super) fn problems(&self) -> Vec<ProblemFile> {
        let Some(scope) = self.scope() else {
            return Vec::new();
        };
        let root = self.root_of(scope);
        let mut files = Vec::new();
        for (held, path, file) in self.open_files() {
            if held != scope || files.iter().any(|seen: &ProblemFile| seen.file == file) {
                continue;
            }
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            let document = document.borrow();
            let problems: Vec<Problem> = document
                .buffer()
                .diagnostics()
                .iter()
                .map(|found| Problem {
                    severity: found.severity,
                    message: found.message.lines().next().unwrap_or_default().to_owned(),
                    source: found.source.clone(),
                    at: found.range.start,
                })
                .collect();
            if problems.is_empty() {
                continue;
            }
            let relative = root
                .as_deref()
                .and_then(|root| path.strip_prefix(root).ok())
                .unwrap_or(&path);
            files.push(ProblemFile {
                file,
                name: relative
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                directory: relative
                    .parent()
                    .map(|parent| parent.display().to_string())
                    .unwrap_or_default(),
                problems,
            });
        }
        files
    }

    /// The debugger of the worktree in front, built when the panel is
    /// showing it.
    pub(super) fn debug_in_panel(&self, theme: &Theme) -> Option<Div<Message>> {
        if !self.showing_debugger() {
            return None;
        }
        let scope = self.scope()?;
        Some(crate::debug::debug_view(
            theme,
            self.debuggers.get(scope),
            self.writing == Some(Writing::Console(scope)),
            self.caret_solid(),
        ))
    }

    /// Scrolls the list of problems by `delta` logical pixels when the
    /// pointer is over it, answering whether it was.
    pub(super) fn scroll_problems(&mut self, delta: f32) -> bool {
        let over = self.tool_visible(Tool::Problems)
            && self
                .pointer
                .is_some_and(|pointer| self.problems_area.get().contains(pointer));
        if over {
            let mut moved = self.problems_scroll.get();
            moved.by(delta);
            self.problems_scroll.set(moved);
        }
        over
    }
}

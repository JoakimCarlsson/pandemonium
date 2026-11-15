//! The keymap's actions about the file showing that the window does rather
//! than its editor: making it read only, linking to its caret line on the
//! repository's host, listing the commits that changed it, opening a shell
//! in its folder, going back and forward between the places jumped from, and
//! setting a breakpoint on its caret line. What needs the repository is
//! asked of the server holding the file, so remote projects work the same.

use pm_editor::Action;
use pm_protocol::{Request, Response};

use super::code::CodeAsk;
use super::{Ask, Link};
use crate::model::DockTab;
use crate::open_files::{Comparison, FileBody, Jump, OpenFile, SessionTab, TabAction};
use crate::quick::{Choosing, QuickInput, QuickItem, QuickKind, QuickOutcome, QuickPick};

impl Link {
    /// Does `action` when it is about the file showing, giving back what it asks of the window beyond the link, or `None` when it is about something else or no file shows.
    pub(super) fn file_key(&mut self, action: Action) -> Option<Option<QuickOutcome>> {
        match action {
            Action::ToggleReadOnly => {
                let index = self.shown_index()?;
                self.file_tab(TabAction::ToggleReadOnly(index));
            }
            Action::CopyPermalink => self.ask_permalink(false)?,
            Action::OpenPermalink => self.ask_permalink(true)?,
            Action::FileHistory => self.ask_file_history()?,
            Action::OpenTerminalHere => self.terminal_here()?,
            Action::ToggleBreakpoint => self.breakpoint_at_caret()?,
            Action::GoBack => {
                if let Some(jump) = self.workspace.open_files.go_back() {
                    self.show_jump(jump);
                }
            }
            Action::GoForward => {
                if let Some(jump) = self.workspace.open_files.go_ahead() {
                    self.show_jump(jump);
                }
            }
            _ => return None,
        }
        Some(None)
    }

    /// The position of the file showing, when a file tab is chosen.
    fn shown_index(&self) -> Option<usize> {
        match self.workspace.open_files.tab {
            SessionTab::File(index) if index < self.workspace.open_files.files.len() => Some(index),
            _ => None,
        }
    }

    /// Asks the server for the web address of the caret line of the file showing, or of the lines selected, on its repository's host, to open in the browser when `open` and to copy otherwise.
    fn ask_permalink(&mut self, open: bool) -> Option<()> {
        let file = self.workspace.open_files.shown()?;
        let (first, last) = caret_lines(file);
        let request = Request::Permalink {
            directory: file.directory.clone(),
            path: file.path.clone(),
            line: Some(first),
            end_line: (last != first).then_some(last),
        };
        self.ask(request, Ask::Code(CodeAsk::Permalink(open)));
        Some(())
    }

    /// Asks the server for the commits that changed the file showing, and opens the quick input to choose one of them once they arrive.
    fn ask_file_history(&mut self) -> Option<()> {
        let file = self.workspace.open_files.shown()?;
        let (directory, path) = (file.directory.clone(), file.path.clone());
        self.workspace.open_files.commits.clear();
        self.workspace.quick = Some(QuickInput {
            choosing: Some(Choosing::Commits),
            waiting: Some(String::new()),
            ..QuickInput::default()
        });
        self.ask(
            Request::FileHistory {
                directory: directory.clone(),
                path: path.clone(),
            },
            Ask::Code(CodeAsk::Commits(directory, path)),
        );
        Some(())
    }

    /// Asks the server what the commit at `place` among those listed for the file showing held of it, to compare the file with.
    pub(super) fn compare_with_commit(&mut self, place: usize) {
        let Some(file) = self.workspace.open_files.shown() else {
            return;
        };
        let Some((object, old_path, label)) = self.workspace.open_files.commits.get(place) else {
            return;
        };
        let ask = CodeAsk::Committed(file.directory.clone(), file.path.clone(), label.clone());
        let request = Request::ReadFile {
            directory: file.directory.clone(),
            path: old_path.clone(),
            head: false,
            revision: Some(object.clone()),
        };
        self.ask(request, Ask::Code(ask));
    }

    /// Starts a shell in the folder of the file showing and shows it in the dock.
    fn terminal_here(&mut self) -> Option<()> {
        let file = self.workspace.open_files.shown()?;
        let folder = file_folder(&file.directory, &file.path);
        self.workspace.dock_open = true;
        self.workspace.dock_tab = DockTab::Terminal;
        self.change_dock(|dock| dock.new_shell_in(folder));
        Some(())
    }

    /// Sets a breakpoint on the caret line of the file showing, or takes the one there away, as pressing its line number does.
    fn breakpoint_at_caret(&mut self) -> Option<()> {
        let file = self.workspace.open_files.shown()?;
        let (line, _) = caret_lines(file);
        let path = file.path.clone();
        self.toggle_breakpoint(path, line.saturating_sub(1));
        Some(())
    }

    /// Shows the file `jump` names with its caret on the line it names, reading the file when it is not open.
    fn show_jump(&mut self, jump: Jump) {
        let files = &mut self.workspace.open_files;
        files.vim = self.settings.editor.vim_mode;
        if files.open(&jump.directory, &jump.path, true).is_some() {
            self.read_file(jump.directory.clone(), jump.path.clone());
        }
        if let SessionTab::File(index) = self.workspace.open_files.tab
            && let Some(file) = self.workspace.open_files.files.get_mut(index)
        {
            file.go_to(jump.line);
        }
    }

    /// Handles the `response` to a request made for `ask`, about a permalink or the commits of a file.
    pub(super) fn answer_file_key(&mut self, ask: CodeAsk, response: Response) {
        match (ask, response) {
            (CodeAsk::Permalink(true), Response::Address { url }) => self.open_browser(Some(&url)),
            (CodeAsk::Permalink(false), Response::Address { url }) => {
                self.workspace.status = Some(format!("Copied {url}"));
                self.workspace.open_files.copied = Some(url);
            }
            (CodeAsk::Commits(directory, path), Response::FileHistory { commits }) => {
                let shown = self
                    .workspace
                    .open_files
                    .shown()
                    .is_some_and(|file| file.directory == directory && file.path == path);
                if !shown {
                    return;
                }
                self.workspace.open_files.commits = commits
                    .iter()
                    .map(|commit| {
                        let label = format!("{} {}", commit.id, commit.summary);
                        (commit.object.clone(), commit.path.clone(), label)
                    })
                    .collect();
                let found = commits.into_iter().enumerate().map(commit_item).collect();
                if let Some(quick) = self
                    .workspace
                    .quick
                    .as_mut()
                    .filter(|quick| quick.choosing == Some(Choosing::Commits))
                {
                    quick.waiting = None;
                    quick.selected = 0;
                    quick.found = found;
                }
            }
            (CodeAsk::Committed(directory, path, label), Response::File { text, .. }) => {
                let Some(index) = self.workspace.open_files.position(&directory, &path) else {
                    return;
                };
                let file = &self.workspace.open_files.files[index];
                if file.body != FileBody::Text {
                    return;
                }
                let now = file.document.0.borrow().field.text().to_owned();
                let then = text.unwrap_or_default();
                self.show_comparison(index, Comparison::between(label, &then, &now));
            }
            (CodeAsk::Commits(..), Response::Failed { message }) => {
                if let Some(quick) = self
                    .workspace
                    .quick
                    .as_mut()
                    .filter(|quick| quick.choosing == Some(Choosing::Commits))
                {
                    quick.waiting = None;
                }
                self.workspace.status = Some(message);
            }
            (_, Response::Failed { message }) => self.workspace.status = Some(message),
            _ => {}
        }
    }
}

/// The row of the quick input listing `commit`, at `place` among the commits of the file: its message's first line, beside its short name, author and age.
fn commit_item((place, commit): (usize, pm_git::FileCommit)) -> QuickItem {
    QuickItem {
        title: commit.summary,
        detail: format!("{} · {} · {}", commit.id, commit.author, commit.when),
        kind: QuickKind::Choice,
        keys: None,
        matched: Vec::new(),
        pick: QuickPick::Choose(Choosing::Commits, place),
    }
}

/// The first and last lines, counted from 1, of the selection of `file`, or the caret's line twice when nothing is selected; the line its caret goes to once its text arrives, or the first, while it has none.
fn caret_lines(file: &OpenFile) -> (usize, usize) {
    if file.body != FileBody::Text {
        let line = file.line.unwrap_or(1);
        return (line, line);
    }
    let document = file.document.0.borrow();
    match document.field.selection() {
        Some(range) if !range.is_empty() => {
            let first = document.line_of(range.start) + 1;
            let last = document.line_of(range.end.saturating_sub(1).max(range.start)) + 1;
            (first, last)
        }
        _ => {
            let line = document.line_of(document.field.caret()) + 1;
            (line, line)
        }
    }
}

/// The folder holding the file at `path`, from the root of `directory`, written whole.
fn file_folder(directory: &str, path: &str) -> String {
    let root = directory.trim_end_matches('/');
    match path.rsplit_once('/') {
        Some((parent, _)) if !parent.is_empty() => format!("{root}/{parent}"),
        _ => root.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file's folder is its directory joined with the folders of its path.
    #[test]
    fn folders_of_files() {
        assert_eq!(file_folder("/w/p/", "src/a/b.rs"), "/w/p/src/a");
        assert_eq!(file_folder("ssh://host/p", "main.rs"), "ssh://host/p");
    }

    /// The caret line of a file whose text has not arrived is the line it goes to.
    #[test]
    fn caret_line_of_loading_file() {
        let file = OpenFile {
            line: Some(4),
            ..OpenFile::default()
        };
        assert_eq!(caret_lines(&file), (4, 4));
    }

    /// Going back after a jump shows the file jumped from at its line, and forward shows the place gone back from.
    #[test]
    fn going_back_and_forward() {
        let mut link = Link::built(None, None, std::sync::Arc::new(|| {}));
        link.workspace.open_files.open("d", "a.rs", true);
        link.workspace.open_files.files[0].line = Some(5);
        link.workspace.open_files.leave();
        link.workspace.open_files.open("d", "b.rs", true);
        assert_eq!(link.run_action(Action::GoBack), Some(None));
        let shown = link.workspace.open_files.shown().expect("a file shows");
        assert_eq!((shown.path.as_str(), shown.line), ("a.rs", Some(5)));
        link.run_action(Action::GoForward);
        let shown = link.workspace.open_files.shown().expect("a file shows");
        assert_eq!(shown.path, "b.rs");
        assert_eq!(link.run_action(Action::ToggleReadOnly), Some(None));
        assert!(link.workspace.open_files.files[1].read_only);
    }
}

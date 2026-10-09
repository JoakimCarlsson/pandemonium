//! Carrying out a named command, whoever asked for it.
//!
//! A keybinding, a menu entry, a button on the search bar and the command
//! palette all resolve to the same [`Action`], and this is the one place any
//! of them is carried out. What a command needs — the focused pane, the file
//! in front of it, the worktree it belongs to — the window has; the screens
//! that asked for it do not.

use pm_text::{Buffer, Position};
use pm_ui::Axis;

use crate::app::App;
use crate::app::places::Place;
use crate::desktop;
use crate::editor::{self, Completions, Document, SearchField};
use crate::keymap::Action;
use crate::message::Message;
use crate::panes::{Item, PaneId, SplitDirection};
use crate::picker::Kind;

/// How far one step of zoom moves the editor's text.
const ZOOM_STEP: f32 = 0.1;

/// The narrowest and widest the editor's text is drawn, as a factor.
const ZOOM_RANGE: (f32, f32) = (0.5, 3.0);

impl App {
    /// Carries `action` out.
    pub(super) fn act(&mut self, action: Action) {
        if self.act_on_field(action) {
            return self.request_redraw();
        }
        if action == Action::Copy && self.copy_reading_text() {
            return self.request_redraw();
        }
        if self.act_on_terminal(action) {
            return self.request_redraw();
        }
        if self.debug_action(action) {
            return self.request_redraw();
        }
        if self.task_action(action) {
            return self.request_redraw();
        }
        match action {
            Action::ShowSessions => self.open_picker(Kind::Sessions),
            Action::RunChecks => {
                if let Some(scope) = self.scope() {
                    self.run_checks(scope, false);
                }
            }
            Action::AcceptPrediction => self.accept_prediction(false),
            Action::AcceptPredictionWord => self.accept_prediction(true),
            Action::DismissPrediction => self.dismiss_prediction(),
            Action::ShowCommands => self.open_picker(Kind::Commands),
            Action::ShowTool(tool) => return self.apply(Message::ShowTool(tool)),
            Action::ResetWindowLayout => return self.apply(Message::ResetWindowLayout),
            Action::InstallLanguageServer => self.open_picker(Kind::LanguageServers),
            Action::RenameTerminal => {
                if let Some(id) = self
                    .scope()
                    .and_then(|scope| self.terminals.active_id(scope))
                {
                    self.open_terminal_rename(id);
                }
            }
            Action::ShowFiles => self.open_picker(Kind::Files),
            Action::ShowProjects => self.open_picker(Kind::Projects),
            Action::SearchProject => self.open_picker(Kind::Search),
            Action::ReplaceProject => self.open_project_search(None),
            Action::ShowProblems => self.open_picker(Kind::Problems),
            Action::OpenServerLog => self.open_server_log(),
            Action::ToggleServerTrace => self.toggle_server_trace(),
            Action::SwitchBranch => self.open_picker(Kind::Branches),
            Action::CreateBranch => self.open_picker(Kind::NewBranch),
            Action::OpenLanguages => {
                self.open_settings();
                self.settings.show(crate::settings::SettingsPage::Languages);
            }
            Action::InstallLanguageExtension => self.open_picker(Kind::LanguageExtensions),
            Action::OpenSettings => return self.apply(Message::OpenSettings),
            Action::OpenKeymap => {
                self.open_settings();
                self.settings.show(crate::settings::SettingsPage::Keymap);
            }
            Action::ToggleSidebar => {
                return self.apply(Message::ToggleTool(crate::panes::Tool::Projects));
            }
            Action::TogglePanel => {
                return self.apply(Message::ToggleTool(crate::panes::Tool::Terminal));
            }
            Action::ToggleSecondarySidebar => {
                return self.apply(Message::ToggleTool(crate::panes::Tool::Files));
            }
            Action::CloseWindow => return self.apply(Message::CloseWindow),
            Action::ToggleFullscreen => self.toggle_fullscreen(),
            Action::AddProject => return self.apply(Message::OpenProject),
            Action::NewSession => return self.apply(Message::NewSession),
            Action::RemoveProject => {
                if let Some(id) = self.open.active().map(pm_core::Project::id) {
                    return self.apply(Message::CloseProject(id));
                }
            }
            Action::Save => self.save_or_format(),
            Action::SaveWithoutFormat => self.save_unformatted(),
            Action::CopyPath | Action::CopyRelativePath | Action::RevealFile => {
                if let Some(message) = self.file_message(action) {
                    return self.apply(message);
                }
            }
            Action::ToggleSearchCase if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.toggle_project_search(crate::message::ProjectSearchOption::Case)
            }
            Action::ToggleSearchCase => {
                return self.apply(Message::ToggleSearchCase(self.panes.focus()));
            }
            Action::ToggleSearchWord if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.toggle_project_search(crate::message::ProjectSearchOption::Word)
            }
            Action::ToggleSearchWord => {
                return self.apply(Message::ToggleSearchWord(self.panes.focus()));
            }
            Action::ToggleSearchRegex if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.toggle_project_search(crate::message::ProjectSearchOption::Regex)
            }
            Action::ToggleSearchRegex => {
                return self.apply(Message::ToggleSearchRegex(self.panes.focus()));
            }
            Action::ToggleSearchReplace => {
                return self.apply(Message::ToggleSearchReplace(self.panes.focus()));
            }
            Action::SaveAll => self.save_all(),
            Action::SplitRight => self.split_pane(self.panes.focus(), None, SplitDirection::Right),
            Action::SplitDown => self.split_pane(self.panes.focus(), None, SplitDirection::Down),
            Action::SplitLeft => self.split_pane(self.panes.focus(), None, SplitDirection::Left),
            Action::SplitUp => self.split_pane(self.panes.focus(), None, SplitDirection::Up),
            Action::CloseSavedTabs => {
                return self.apply(Message::CloseSavedTabs(self.panes.focus()));
            }
            Action::CloseAllTabs => return self.apply(Message::CloseAllTabs(self.panes.focus())),
            Action::CloseOtherTabs
            | Action::CloseTabsLeft
            | Action::CloseTabsRight
            | Action::TogglePin => {
                if let Some(message) = self.tab_message(action) {
                    return self.apply(message);
                }
            }
            Action::ActivateTab(place) => self.activate_tab_at(Some(usize::from(place))),
            Action::ActivateLastTab => self.activate_tab_at(None),
            Action::ClosePane => self.close_active_tab(),
            Action::ReopenTab => self.reopen_tab(),
            Action::NextTab | Action::PreviousTab => {
                let pane = self.panes.focus();
                let along = if action == Action::NextTab { 1 } else { -1 };
                let scope = self.scope();
                if let Some(file) =
                    scope.and_then(|scope| self.panes.pane(pane)?.tab_along(scope, along))
                {
                    self.activate_tab(pane, file);
                }
            }
            Action::FocusLeft => self.focus_neighbour(Axis::Horizontal, false),
            Action::FocusRight => self.focus_neighbour(Axis::Horizontal, true),
            Action::FocusUp => self.focus_neighbour(Axis::Vertical, false),
            Action::FocusDown => self.focus_neighbour(Axis::Vertical, true),
            Action::NewTerminal => {
                self.start_shell();
                self.terminal_focused = true;
                self.editor_focused = false;
            }
            Action::ShowChanges => {
                return self.apply(Message::ShowTool(crate::panes::Tool::Changes));
            }
            Action::FocusFiles => self.reveal_in_tree(),
            Action::NewFile => return self.apply(Message::NewTreeFile),
            Action::NewFolder => return self.apply(Message::NewTreeFolder),
            Action::CollapseFiles => return self.apply(Message::CollapseTree),
            Action::CompareTurns => self.compare_turns(),
            Action::OpenReview => return self.apply(Message::OpenReview),
            Action::EditChanges | Action::ReviewSession => {
                return self.apply(Message::OpenExcerpts);
            }
            Action::OpenMarkdownPreview => self.open_rendered(),
            Action::ShowOutline => self.open_outline(),
            Action::NewAgentSession => return self.apply(Message::NewAgentSession),
            Action::AddSelectionToAgent => self.add_selection_to_agent(),
            Action::AddReviewComment => return self.apply(Message::AddComment),
            Action::FinishSession => {
                if let Some(session) = self.selected_session() {
                    return self.apply(Message::FinishSession(session));
                }
            }
            Action::ChangeAgentMode => {
                if let Some(session) = self.focused_talk() {
                    return self.apply(Message::ShowAgentModes(session));
                }
            }
            Action::ToggleAgentDetails => {
                if let Some(session) = self.focused_talk()
                    && let Some(talk) = self.agents.get_mut(session)
                {
                    talk.toggle_all_details();
                }
            }
            Action::CycleAgentMode => {
                if let Some(session) = self.focused_talk() {
                    return self.apply(Message::CycleAgentMode(session));
                }
            }
            Action::ChangeAgentModel => {
                if let Some(session) = self.focused_talk()
                    && let Some(place) = self.knob_about(session, pm_acp::About::Model)
                {
                    return self.apply(Message::PressKnob(session, place));
                }
            }
            Action::ReconnectAgent => {
                if let Some(session) = self.focused_talk() {
                    return self.apply(Message::ReconnectAgent(session));
                }
            }
            Action::LogOutAgent => {
                if let Some(session) = self.focused_talk() {
                    return self.apply(Message::LogOutAgent(session));
                }
            }
            Action::DeleteAgentSession => {
                if let Some(session) = self.focused_talk() {
                    return self.apply(Message::ShowAgentDeletions(session));
                }
            }
            Action::StageSelectedChanges => return self.apply(Message::StageSelection),
            Action::UnstageSelectedChanges => return self.apply(Message::UnstageSelection),
            Action::DiscardSelectedChanges => return self.apply(Message::DiscardSelection),
            Action::StageAllChanges => return self.apply(Message::StageAll),
            Action::UnstageAllChanges => return self.apply(Message::UnstageAll),
            Action::CommitChanges => return self.apply(Message::Commit),
            Action::RefreshChanges => return self.apply(Message::RefreshChanges),
            Action::Cancel => self.cancel(),
            Action::ZoomIn => self.zoom_by(ZOOM_STEP),
            Action::ZoomOut => self.zoom_by(-ZOOM_STEP),
            Action::ZoomReset => self.zoom = 1.0,
            _ => self.act_on_buffer(action),
        }
        self.request_redraw();
    }

    /// What asking for `action` of the tab the focused pane is showing
    /// tells the window, when the pane is showing one.
    fn tab_message(&self, action: Action) -> Option<Message> {
        let pane = self.panes.focus();
        let item = self.active_tab()?;
        Some(match action {
            Action::CloseOtherTabs => Message::CloseOtherTabs(pane, item),
            Action::CloseTabsLeft => Message::CloseTabsLeft(pane, item),
            Action::CloseTabsRight => Message::CloseTabsRight(pane, item),
            _ => Message::TogglePin(pane, item),
        })
    }

    /// What asking for `action` of the file the focused pane is showing
    /// tells the window, when the pane is showing one.
    fn file_message(&self, action: Action) -> Option<Message> {
        let file = self.active_file_id()?;
        Some(match action {
            Action::CopyPath => Message::CopyFilePath(file),
            Action::CopyRelativePath => Message::CopyFileRelativePath(file),
            _ => Message::RevealFile(file),
        })
    }

    /// Shows the tab in `place` of the focused pane, or its last tab for
    /// no place.
    fn activate_tab_at(&mut self, place: Option<usize>) {
        let pane = self.panes.focus();
        let Some(tabs) = self.panes.pane(pane).map(|held| held.tabs(self.scope())) else {
            return;
        };
        let chosen = match place {
            Some(place) => tabs.get(place),
            None => tabs.last(),
        };
        if let Some(item) = chosen.copied() {
            self.activate_tab(pane, item);
        }
    }

    /// Fills the screen with the window, or gives the screen back.
    fn toggle_fullscreen(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let filling = match window.fullscreen() {
            Some(_) => None,
            None => Some(winit::window::Fullscreen::Borderless(None)),
        };
        window.set_fullscreen(filling);
    }

    /// Routes editing actions to the shared input that has the keyboard.
    pub(super) fn act_on_field(&mut self, action: Action) -> bool {
        if !crate::input::Input::handles(action) {
            return false;
        }
        if action == Action::Paste && self.picker.is_none() && self.paste_prompt() {
            return true;
        }
        let picker = self.picker.is_some();
        let mut copied = None;
        let mut changed = false;
        if !self.edit_focused_field(|input| {
            let before = input.value();
            match action {
                Action::Copy => copied = input.selected_text(),
                Action::Cut => copied = input.cut_selection(),
                Action::Paste => {
                    if let Some(text) = desktop::paste() {
                        input.paste(&text);
                    }
                }
                _ => {
                    input.act(action);
                }
            }
            changed = before != input.value();
        }) {
            return false;
        }
        if let Some(text) = copied {
            desktop::copy(text);
        }
        if changed {
            if picker {
                if let Some(picker) = self.picker.as_mut() {
                    picker.filter();
                }
                self.refilter_picker();
            } else {
                self.input_retyped();
            }
        }
        true
    }

    /// Carries out a command that acts on the file the focused pane shows.
    fn act_on_buffer(&mut self, action: Action) {
        if let Some(edit) = editor::action_edit(action, self.page_rows()) {
            self.apply_edit(edit);
            return;
        }
        match action {
            Action::Undo => self.edit_active(|buffer| {
                buffer.undo();
            }),
            Action::Redo => self.edit_active(|buffer| {
                buffer.redo();
            }),
            Action::Cut => {
                if let Some(text) = self.with_buffer(Buffer::copied_text) {
                    desktop::copy(text);
                }
                self.edit_active(|buffer| buffer.at_each(Buffer::cut));
            }
            Action::Copy => {
                if let Some(text) = self.with_buffer(Buffer::copied_text) {
                    desktop::copy(text);
                }
            }
            Action::Paste => {
                if let Some(text) = desktop::paste() {
                    self.edit_active(|buffer| buffer.at_each(|buffer| buffer.paste(&text)));
                }
            }
            Action::SelectAll => self.edit_active(Buffer::select_all),
            Action::SelectLine => self.edit_active(|buffer| {
                let head = buffer.selection().head;
                buffer.select_line(head);
            }),
            Action::ExpandSelection => self.expand_selection(),
            Action::AddCursorAbove => {
                self.edit_active(|buffer| buffer.add_cursor_vertically(false))
            }
            Action::AddCursorBelow => self.edit_active(|buffer| buffer.add_cursor_vertically(true)),
            Action::AddNextMatch => self.edit_active(Buffer::add_next_match),
            Action::SelectAllMatches => self.edit_active(Buffer::select_all_matches),
            Action::CollapseCursors => self.edit_active(|buffer| {
                buffer.collapse_cursors();
            }),
            Action::DuplicateLine => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::duplicate_lines))
            }
            Action::DeleteLine => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::delete_lines))
            }
            Action::MoveLineUp => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::move_lines_up))
            }
            Action::MoveLineDown => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::move_lines_down))
            }
            Action::JoinLines => self.edit_active(|buffer| buffer.on_each_line(Buffer::join_lines)),
            Action::InsertLineBelow => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::insert_line_below));
            }
            Action::InsertLineAbove => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::insert_line_above));
            }
            Action::ToggleComment => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::toggle_comment))
            }
            Action::Indent => self.edit_active(|buffer| buffer.on_each_line(Buffer::indent_lines)),
            Action::Outdent => {
                self.edit_active(|buffer| buffer.on_each_line(Buffer::outdent_lines))
            }
            Action::Find => self.open_search(false, None),
            Action::Replace => self.open_search(true, None),
            Action::FindSelection => {
                let selected = self.with_buffer(Buffer::selected_text);
                self.open_search(false, selected);
            }
            Action::FindNext if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.step_project_match(true)
            }
            Action::FindNext => self.step_search(true),
            Action::FindPrevious if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.step_project_match(false)
            }
            Action::FindPrevious => self.step_search(false),
            Action::ReplaceMatch if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.replace_project(false, false)
            }
            Action::ReplaceMatch => self.replace_match(),
            Action::ReplaceInFile => self.replace_project(true, false),
            Action::ReplaceAll if matches!(self.active_tab(), Some(Item::Search(_))) => {
                self.ask_project_replace_all()
            }
            Action::ReplaceAll => self.replace_all(),
            Action::GoToLine => self.open_prompt(action),
            Action::NextDiagnostic => self.step_diagnostic(true),
            Action::PreviousDiagnostic => self.step_diagnostic(false),
            Action::GoBack => self.travel(true),
            Action::GoForward => self.travel(false),
            Action::ToggleFold => {
                let line = self.with_buffer(|buffer| buffer.selection().head.line);
                if let Some(line) = line {
                    self.with_document(|document| document.toggle_fold(line));
                }
            }
            Action::FoldAll => {
                self.with_document(Document::fold_all);
            }
            Action::UnfoldAll => {
                self.with_document(Document::unfold_all);
            }
            Action::ToggleBlame => self.toggle_blame(),
            Action::NextChange => self.step_change(true),
            Action::PreviousChange => self.step_change(false),
            Action::RevertChange => self.revert_change(),
            _ => self.act_on_language(action),
        }
    }

    /// Lets the servers behind the focused file change it, and writes it to disk.
    ///
    /// What a server changes before a save is asked of it and answered
    /// later, so a save it takes part in is the questions now and the writing
    /// when the answers land. Only the first step is taken here, or the
    /// writing would ask again and never settle.
    fn save_or_format(&mut self) {
        let served = self
            .active_file()
            .is_some_and(|document| document.borrow().is_served());
        if served {
            return self.begin_save(self.active_language_settings().format_on_save);
        }
        self.format_locally(true);
        self.save_active();
    }

    /// Lets the servers behind the focused file change it without laying it
    /// out, and writes it to disk.
    fn save_unformatted(&mut self) {
        let served = self
            .active_file()
            .is_some_and(|document| document.borrow().is_served());
        if served {
            return self.begin_save(false);
        }
        self.save_active();
    }

    /// How many lines a page of the focused file is.
    fn page_rows(&self) -> usize {
        self.active_file().map_or(1, |file| file.borrow().rows())
    }

    /// Writes the file the focused pane is showing to disk.
    pub(super) fn save_active(&mut self) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(root) = self.worktree_of(file) else {
            return;
        };
        self.editor.save(file, &root);
        self.reread_changes();
        self.follow_server_settings();
    }

    /// Writes every changed file to disk, each into its own worktree.
    fn save_all(&mut self) {
        let roots = self
            .scopes()
            .into_iter()
            .filter_map(|scope| Some((scope, self.root_of(scope)?)))
            .collect::<Vec<_>>();
        self.editor.save_all(&|scope| {
            roots
                .iter()
                .find(|(held, _)| *held == scope)
                .map(|(_, root)| root.clone())
        });
        self.reread_changes();
        self.follow_server_settings();
    }

    /// The worktree the file `id` names was opened from.
    pub(super) fn worktree_of(&self, id: crate::editor::FileId) -> Option<std::path::PathBuf> {
        match self.editor.scope_of(id) {
            Some(scope) => self.root_of(scope),
            None => self
                .editor
                .path(id)?
                .parent()
                .map(std::path::Path::to_path_buf),
        }
    }

    /// Puts what the pointer is carrying down where the cursor is.
    ///
    /// Zooming is the window's, not the document's: every pane draws its
    /// text at the size the reader last asked for, because the size is a
    /// preference and not a property of a file.
    fn zoom_by(&mut self, step: f32) {
        self.zoom = (self.zoom + step).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
    }

    /// Dismisses whatever is open on top, innermost first.
    fn cancel(&mut self) {
        if matches!(
            self.writing,
            Some(crate::app::Writing::LanguageServerField(_))
        ) {
            self.writing = None;
            self.languages.editor = None;
            return;
        }
        if self.dismiss_prompt() {
            return;
        }
        if self.dismiss_menu() {
            return;
        }
        if self.changes_focused {
            if !self.clear_change_marks() {
                self.changes_focused = false;
            }
            return;
        }
        if self.release_commit_focus() {
            return;
        }
        if self.release_comment_focus() {
            return;
        }
        if self.stop_or_release_prompt() {
            return;
        }
        if self.with_buffer(Buffer::has_many_cursors) == Some(true) {
            return self.edit_active(|buffer| {
                buffer.collapse_cursors();
            });
        }
        if self.dismiss_popup() {
            return;
        }
        if self.dismiss_picker() {
            return;
        }
        if self.close_search() {
            return;
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.clear_focus();
        }
    }

    /// Opens the search bar over the focused pane.
    fn open_search(&mut self, replacing: bool, seeded: Option<String>) {
        let Some(file) = self.active_file() else {
            return;
        };
        let seeded = seeded.or_else(|| {
            let selected = file.borrow().buffer().selected_text();
            (!selected.is_empty()).then_some(selected)
        });
        file.borrow_mut()
            .search_with(|search, buffer| search.open(seeded, replacing, buffer));
        self.search_focused = true;
    }

    /// Closes the search bar, saying whether one was open.
    pub(super) fn close_search(&mut self) -> bool {
        let Some(file) = self.active_file() else {
            return false;
        };
        let open = file.borrow().search().is_open();
        if open {
            file.borrow_mut().search_with(|search, _| search.close());
            self.search_focused = false;
        }
        open
    }

    /// Goes to the match after the one being looked at, or the one before.
    fn step_search(&mut self, forward: bool) {
        let Some(file) = self.active_file() else {
            return;
        };
        if !file.borrow().search().is_open() {
            return self.open_search(false, None);
        }
        let mut document = file.borrow_mut();
        let found = if forward {
            document.search_next()
        } else {
            document.search_previous()
        };
        if let Some(found) = found {
            document.edit(|buffer| buffer.select_range(found));
        }
    }

    /// Puts the replacement in place of the match being looked at.
    fn replace_match(&mut self) {
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let Some(found) = document.search().current() else {
            return;
        };
        let Ok(finder) = document.search().finder() else {
            return;
        };
        let replacement = finder.replacement(
            &document.buffer().line_text(found.start.line),
            found.start.column..found.end.column,
            &document.search().replacement().value(),
        );
        document.edit(|buffer| buffer.replace(found, &replacement));
        document.search_with(|search, buffer| search.refresh(buffer));
    }

    /// Puts the replacement in place of every match at once.
    fn replace_all(&mut self) {
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let found = document.search().matches().to_vec();
        if found.is_empty() {
            return;
        }
        let Ok(finder) = document.search().finder() else {
            return;
        };
        let edits = found
            .into_iter()
            .map(|range| {
                let replacement = finder.replacement(
                    &document.buffer().line_text(range.start.line),
                    range.start.column..range.end.column,
                    &document.search().replacement().value(),
                );
                (range, replacement)
            })
            .collect();
        document.edit(|buffer| buffer.apply_edits(edits));
        document.search_with(|search, buffer| search.refresh(buffer));
    }

    /// Goes to the next error or warning in the file, or the previous one.
    fn step_diagnostic(&mut self, forward: bool) {
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let head = document.buffer().selection().head;
        let mut found = document
            .buffer()
            .diagnostics()
            .iter()
            .map(|found| found.range.start)
            .collect::<Vec<_>>();
        found.sort_unstable();
        let next = if forward {
            found.iter().find(|at| **at > head).or(found.first())
        } else {
            found.iter().rev().find(|at| **at < head).or(found.last())
        };
        let Some(next) = next.copied() else {
            return;
        };
        document.edit(|buffer| buffer.place(next, false));
    }

    /// Shows who last changed each line of the focused file, or stops.
    ///
    /// Blaming a file means asking git, which is a subprocess over a whole
    /// file's history: it is asked on a thread of its own and the column
    /// fills in when the answer comes back.
    fn toggle_blame(&mut self) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let shown = document.borrow().is_blamed();
        document.borrow_mut().show_blame(!shown);
        if shown || !document.borrow().blame().is_empty() {
            return;
        }

        let Some(root) = self.worktree_of(file) else {
            return;
        };
        let path = document.borrow().buffer().path().to_path_buf();
        let blamed = self.blamed.clone();
        let wake = self.waker(crate::app::Wake::Blame);
        std::thread::spawn(move || {
            let lines = pm_core::blame(&root, &path);
            if let Ok(mut blamed) = blamed.lock() {
                blamed.push((file, lines));
            }
            wake();
        });
    }

    /// Takes in whatever blame has come back, saying whether any had.
    pub(super) fn collect_blame(&mut self) -> bool {
        let Ok(mut blamed) = self.blamed.lock() else {
            return false;
        };
        let ready = std::mem::take(&mut *blamed);
        drop(blamed);
        if ready.is_empty() {
            return false;
        }

        for (file, lines) in ready {
            if let Some(document) = self.editor.get(file) {
                document.borrow_mut().set_blame(lines);
            }
        }
        true
    }

    /// Goes to the next place the file differs from the index, or the previous.
    fn step_change(&mut self, forward: bool) {
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let head = document.buffer().selection().head.line;
        let anchors = document
            .changes()
            .iter()
            .map(pm_core::Change::anchor)
            .collect::<Vec<_>>();
        let next = if forward {
            anchors
                .iter()
                .find(|line| **line > head)
                .or(anchors.first())
        } else {
            anchors
                .iter()
                .rev()
                .find(|line| **line < head)
                .or(anchors.last())
        };
        let Some(line) = next.copied() else {
            return;
        };
        document.edit(|buffer| buffer.place(Position::new(line, 0), false));
    }

    /// Puts the change the cursor is in back the way the index has it.
    fn revert_change(&mut self) {
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let head = document.buffer().selection().head.line;
        let Some(change) = document
            .changes()
            .iter()
            .find(|change| change.covers(head))
            .cloned()
        else {
            return;
        };

        document.edit(|buffer| {
            let start = Position::new(change.lines.start, 0);
            let end = match change.lines.end < buffer.line_count() {
                true => Position::new(change.lines.end, 0),
                false => Position::new(
                    buffer.line_count().saturating_sub(1),
                    buffer.line_len(buffer.line_count().saturating_sub(1)),
                ),
            };
            buffer.replace(start..end, &change.removed);
        });
    }

    /// Goes back to where the cursor was before the last jump, or forward.
    fn travel(&mut self, back: bool) {
        let Some(from) = self.here() else {
            return;
        };
        let place = if back {
            self.trail.back(from)
        } else {
            self.trail.forward(from)
        };
        if let Some(place) = place {
            self.go_to(&place);
        }
    }

    /// Where the cursor is, as a place the trail can bring the window back to.
    pub(super) fn here(&self) -> Option<Place> {
        self.place_in(self.panes.focus())
    }

    /// Where the cursor is in `pane`, as a place the trail can return to.
    pub(super) fn place_in(&self, pane: PaneId) -> Option<Place> {
        let file = self.panes.pane(pane)?.active(self.scope()?)?.file()?;
        let document = self.editor.get(file)?;
        let document = document.borrow();
        Some(Place {
            scope: self.editor.scope_of(file)?,
            path: document.buffer().path().to_path_buf(),
            position: document.buffer().selection().head,
        })
    }

    /// Opens `place` in the focused pane and puts the cursor where it names.
    pub(super) fn go_to(&mut self, place: &Place) {
        let Some(root) = self.root_of(place.scope) else {
            return;
        };
        if self.open_picture(self.panes.focus(), place.scope, &place.path, false) {
            return;
        }
        let Some(file) = self.editor.open(place.scope, &root, &place.path, false) else {
            return;
        };
        self.show_file(self.panes.focus(), file, false);
        if let Some(document) = self.editor.get(file) {
            document
                .borrow_mut()
                .edit(|buffer| buffer.place(place.position, false));
        }
    }

    /// Opens `place`, taking down where the cursor was so it can come back.
    pub(super) fn jump_to(&mut self, place: &Place) {
        if let Some(from) = self.here() {
            self.trail.jumped(from);
        }
        self.go_to(place);
    }

    /// Opens again the tab that was closed last.
    fn reopen_tab(&mut self) {
        if let Some(place) = self.trail.reopen() {
            self.go_to(&place);
        }
    }

    /// Reads something off the focused buffer, when a pane is showing one.
    pub(super) fn with_buffer<T>(&self, read: impl FnOnce(&Buffer) -> T) -> Option<T> {
        let document = self.typed_into().or_else(|| self.active_file())?;
        let document = document.borrow();
        Some(read(document.buffer()))
    }

    /// Puts the focused document through `change`, when a pane is showing one.
    pub(super) fn with_document(&mut self, change: impl FnOnce(&mut Document)) -> bool {
        let Some(document) = self.active_file() else {
            return false;
        };
        change(&mut document.borrow_mut());
        true
    }

    /// Puts the `place`-th completion offered into the buffer, over the rest
    /// of the word the cursor is in when `over` is set and the server named
    /// a span for it.
    pub(super) fn take_completion(&mut self, place: usize, over: bool) {
        let Some(completions) = self.completions.as_ref() else {
            return;
        };
        let Some((client, item)) = completions
            .at_place(place)
            .map(|(client, item)| (client.cloned(), item.clone()))
        else {
            return;
        };
        let start = completions.start();
        let waiting = completions.is_asked(&item.handle);
        let callable = item.kind.callable();
        let mut show_signature = false;
        self.completions = None;
        self.recent_completions.remember(&item);
        self.dismiss_prediction();
        self.edit_active(|buffer| {
            buffer.grouped(|buffer| {
                let head = buffer.selection().head;
                let end = item.replace.as_ref().filter(|_| over).map_or(head, |span| {
                    match span.end.line == head.line {
                        true => span.end.max(head),
                        false => head,
                    }
                });
                let base = buffer.complete(start..end, &item.insert, item.extra.clone());
                if !item.stops.is_empty() {
                    buffer.begin_snippet(base, item.stops.clone());
                }
                if callable {
                    if item.insert.contains('(') {
                        if item.insert.ends_with("()") && buffer.selection().is_empty() {
                            let head = buffer.selection().head;
                            if head.column > 0
                                && buffer.char_at(Position::new(head.line, head.column - 1))
                                    == Some(')')
                            {
                                buffer.place(Position::new(head.line, head.column - 1), false);
                            }
                        }
                        show_signature = true;
                    } else if item.stops.is_empty() {
                        let head = buffer.selection().head;
                        if buffer.char_at(head) == Some('(') {
                            buffer.place(Position::new(head.line, head.column + 1), false);
                        } else {
                            buffer.insert_typed('(');
                        }
                        show_signature = true;
                    }
                }
            });
        });
        if let Some(client) = client {
            self.await_taken_completion(client, item, start, waiting);
        }
        if show_signature {
            self.ask(pm_text::Request::Signature);
        }
    }

    /// Follows a keystroke through: narrows the list, or asks for a new one.
    ///
    /// Typing a word, or a character a server said it completes after, is
    /// how completion is asked for in practice; the command exists for the
    /// times it is not offered, not as the only way to see it. While a list
    /// is up, only the servers that said theirs was not all there was are
    /// asked again.
    pub(super) fn after_typing(&mut self, typed: Option<char>) {
        self.narrow_completions();
        self.signature_after_typing(typed);
        let Some(typed) = typed else {
            return;
        };
        self.format_on_type(typed);
        if let Some(list) = self.completions.as_ref() {
            let again = list.incomplete().to_vec();
            self.ask_incomplete(again);
            return;
        }
        if self.completes_after(typed) {
            self.ask(pm_text::Request::Completions(pm_text::Trigger::Character(
                typed,
            )));
        } else if typed == '.' || typed.is_alphanumeric() || typed == '_' {
            if self.active_file_has_servers() {
                self.ask(pm_text::Request::Completions(pm_text::Trigger::Invoked));
            } else {
                self.offer_words();
            }
        }
    }

    /// Narrows the completions to what has been typed since they were offered.
    ///
    /// A cursor that has left the word they were offered for is a cursor
    /// they no longer say anything about, so the list goes away rather than
    /// following it.
    pub(super) fn narrow_completions(&mut self) {
        let Some(start) = self.completions.as_ref().map(Completions::start) else {
            return;
        };
        let typed = self.with_buffer(|buffer| {
            let head = buffer.selection().head;
            (head.line == start.line && head.column >= start.column)
                .then(|| buffer.text_in(start..head))
        });

        match typed.flatten() {
            Some(typed) => {
                if let Some(completions) = self.completions.as_mut() {
                    completions.narrow(&typed);
                }
                if self.completions.as_ref().is_some_and(Completions::is_empty) {
                    self.completions = None;
                }
                self.resolve_completion();
            }
            None => self.completions = None,
        }
    }

    /// Puts away the completions and the hint, saying whether either was up.
    pub(super) fn dismiss_popup(&mut self) -> bool {
        self.completions.take().is_some() | self.hint.take().is_some()
    }

    /// Sends later keystrokes to `field` of the focused pane's search bar.
    pub(super) fn focus_search(&mut self, field: SearchField) {
        self.search_focused = true;
        self.with_document(|document| document.search_with(|search, _| search.focus(field)));
    }

    /// Puts the cursor at the head of the selection, for a motion of its own.
    pub(super) fn place_cursor(&mut self, position: Position, extend: bool) {
        self.edit_active(|buffer| buffer.place(position, extend));
    }
}

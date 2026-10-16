//! What the screen does for writing code: the file shown edited as its language says, what its language server answers, and the languages installed on this machine.

use pm_protocol::{Completion, Diagnostic, ExtensionInfo, Hover, Position};

use crate::screen::Screen;
use crate::workspace::WorkspaceState;

impl Screen {
    /// The screen with `change` made to the workspace, when the workspace is showing.
    fn in_workspace(self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(change(state)),
            onboarding => onboarding,
        }
    }

    /// Whether the workspace is showing and `test` holds of it.
    fn workspace_is(&self, test: impl FnOnce(&WorkspaceState) -> bool) -> bool {
        matches!(self, Self::Workspace(state) if test(state))
    }

    /// The screen after the grammar of `language` was loaded.
    pub fn with_grammar(self, language: &str) -> Self {
        self.in_workspace(|state| state.with_grammar(language))
    }

    /// The screen opening again for its language server every open file of a project on `host` it was told of.
    pub fn reopen_documents(self, host: &Option<String>) -> Self {
        self.in_workspace(|state| state.reopen_documents(host))
    }

    /// The screen asking for the grammars of the languages named `names`.
    pub fn grammars_wanted(self, names: Vec<String>) -> Self {
        self.in_workspace(|state| state.grammars_wanted(names))
    }

    /// The screen after the language server opened the file at `path` of the session at `session`, or could not.
    pub fn with_opened(
        self,
        session: usize,
        path: &str,
        outcome: Result<pm_protocol::Triggers, String>,
    ) -> Self {
        self.in_workspace(|state| state.with_opened(session, path, outcome))
    }

    /// The screen after the language server of `host` said what it finds wrong with the file at `path` of the folder `directory`.
    pub fn with_diagnostics(
        self,
        host: &Option<String>,
        directory: &str,
        path: &str,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        self.in_workspace(|state| state.with_diagnostics(host, directory, path, diagnostics))
    }

    /// The screen after the language server said what could be written in the file at `path` of the session at `session`.
    pub fn with_completions(
        self,
        session: usize,
        path: &str,
        version: i32,
        items: Vec<Completion>,
    ) -> Self {
        self.in_workspace(|state| state.with_completions(session, path, version, items))
    }

    /// The screen after the language server said what it knows about `position` of the file at `path` of the session at `session`.
    pub fn with_hover(
        self,
        session: usize,
        path: &str,
        position: Position,
        hover: Option<Hover>,
    ) -> Self {
        self.in_workspace(|state| state.with_hover(session, path, position, hover))
    }

    /// The screen asking for what could be written at the caret of the file shown, the list hanging at `at`.
    pub fn completion_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.completion_requested(at))
    }

    /// The screen with the list of completions hanging at `at`.
    pub fn with_completion_at(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.with_completion_at(at))
    }

    /// Whether the list of completions is open over the file shown.
    pub fn completion_open(&self) -> bool {
        self.workspace_is(WorkspaceState::completion_open)
    }

    /// The screen with the pick of the list of completions moved `delta` rows.
    pub fn completion_moved(self, delta: isize) -> Self {
        self.in_workspace(|state| state.completion_moved(delta))
    }

    /// The screen with the list of completions closed.
    pub fn completion_closed(self) -> Self {
        self.in_workspace(WorkspaceState::completion_closed)
    }

    /// The screen with the completion picked written in.
    pub fn completion_taken(self) -> Self {
        self.in_workspace(|state| state.completion_taken(None))
    }

    /// The screen showing what is known about the byte `offset` of the file shown, on a card at `at`.
    pub fn hover_requested(self, offset: usize, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.hover_requested(offset, at))
    }

    /// The screen after the card about a place is scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_hover(self, delta: f32, room: f32) -> Self {
        self.in_workspace(|state| state.scrolled_hover(delta, room))
    }

    /// The screen gone back to where the caret was before the last jump.
    pub fn jumped_back(self) -> Self {
        self.in_workspace(WorkspaceState::jumped_back)
    }

    /// The screen gone forward again to where the caret was before the last step back.
    pub fn jumped_forward(self) -> Self {
        self.in_workspace(WorkspaceState::jumped_forward)
    }

    /// The screen with the file shown formatted as its language says, without writing it.
    pub fn formatted_by_hand(self) -> Self {
        self.in_workspace(WorkspaceState::formatted_by_hand)
    }

    /// The screen after the language server gave the edits that format a file.
    pub fn with_formatted(
        self,
        session: usize,
        path: &str,
        version: i32,
        edits: Vec<pm_protocol::TextChange>,
        save: bool,
    ) -> Self {
        self.in_workspace(|state| state.with_formatted(session, path, version, edits, save))
    }

    /// The screen noting the word of the file shown the pointer is on while Ctrl is held.
    pub fn with_reach(self, reach: Option<std::ops::Range<usize>>) -> Self {
        self.in_workspace(|state| state.with_reach(reach))
    }

    /// The screen looking for what `seek` names from the byte `offset` of the file shown, a list of what is found hanging at `at`.
    pub fn seek_requested(self, offset: usize, seek: pm_protocol::Seek, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.seek_requested(offset, seek, at))
    }

    /// The screen after the language server named the places `seek` looks for from a place in a file of the session at `session`.
    pub fn with_places(
        self,
        session: usize,
        seek: pm_protocol::Seek,
        locations: Vec<pm_protocol::Location>,
    ) -> Self {
        self.in_workspace(|state| state.with_places(session, seek, locations))
    }

    /// The screen listing the files of the project the review panel is about at `at`, to open one.
    pub fn file_finder_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.file_finder_requested(at))
    }

    /// The screen listing the symbols of the project at `at`, as the language server of the file shown finds them.
    pub fn symbols_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.symbols_requested(at))
    }

    /// The screen asking who last changed the line of the caret of the file shown.
    pub fn blame_requested(self) -> Self {
        self.in_workspace(WorkspaceState::blame_requested)
    }

    /// The screen after the server said who last changed a line of a file.
    pub fn with_blame(self, session: usize, path: &str, line: usize, blame: pm_git::Blame) -> Self {
        self.in_workspace(|state| state.with_blame(session, path, line, blame))
    }

    /// The screen listing what is wrong with the files of the project, at `at`.
    pub fn problems_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.problems_requested(at))
    }

    /// The screen after the server said what is wrong with the files of the project of the session at `session`.
    pub fn with_problems(self, session: usize, problems: Vec<pm_protocol::Problem>) -> Self {
        self.in_workspace(|state| state.with_problems(session, problems))
    }

    /// The screen listing the lines of the files of the project that hold what is typed, at `at`.
    pub fn search_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.search_requested(at))
    }

    /// The screen after the language server named the symbols of the project matching `query`.
    pub fn with_symbols(
        self,
        session: usize,
        query: &str,
        locations: Vec<pm_protocol::Location>,
    ) -> Self {
        self.in_workspace(|state| state.with_symbols(session, query, locations))
    }

    /// The screen listing the items of the outline of the file shown at `at`.
    pub fn outline_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.outline_requested(at))
    }

    /// Whether a list hangs over the file shown.
    pub fn listing_open(&self) -> bool {
        self.workspace_is(WorkspaceState::listing_open)
    }

    /// The screen with the pick of the list over the file moved `delta` rows.
    pub fn listing_moved(self, delta: isize) -> Self {
        self.in_workspace(|state| state.listing_moved(delta))
    }

    /// The screen with the list over the file closed.
    pub fn listing_closed(self) -> Self {
        self.in_workspace(WorkspaceState::listing_closed)
    }

    /// The screen with the row picked in the list over the file taken.
    pub fn listing_taken(self) -> Self {
        self.in_workspace(|state| state.listing_taken(None))
    }

    /// The screen after the server could not fill the list over the file, for `problem`.
    pub fn listing_failed(self, problem: String) -> Self {
        self.in_workspace(|state| state.listing_failed(problem))
    }

    /// The screen with a box at `at` to give the thing at the byte `offset` of the file shown a new name.
    pub fn rename_requested(self, offset: usize, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.rename_requested(offset, at))
    }

    /// Whether the box for a new name is open.
    pub fn renaming_open(&self) -> bool {
        self.workspace_is(WorkspaceState::renaming_open)
    }

    /// The screen with the box for a new name closed.
    pub fn rename_closed(self) -> Self {
        self.in_workspace(WorkspaceState::rename_closed)
    }

    /// The screen after renaming failed for `problem`.
    pub fn rename_failed(self, problem: String) -> Self {
        self.in_workspace(|state| state.rename_failed(problem))
    }

    /// The screen asking the language server what changes it offers at the caret of the file shown, listed at `at`.
    pub fn code_actions_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.code_actions_requested(at))
    }

    /// The screen after the language server said what changes it offers for the file at `path` of the session at `session`.
    pub fn with_code_actions(
        self,
        session: usize,
        path: &str,
        offered: Vec<pm_protocol::CodeAction>,
    ) -> Self {
        self.in_workspace(|state| state.with_code_actions(session, path, offered))
    }

    /// The screen after an edit across the project of the session at `session` changed its open files by `files`.
    pub fn with_edited_files(self, session: usize, files: Vec<pm_protocol::FileEdit>) -> Self {
        self.in_workspace(|state| state.with_edited_files(session, files))
    }

    /// Whether keys are being recorded for an action on the keymap page.
    pub fn recording_keys(&self) -> bool {
        self.workspace_is(WorkspaceState::recording_keys)
    }

    /// The screen showing the chords pressed so far while keys are recorded.
    pub fn with_recorded(self, chords: Vec<String>) -> Self {
        self.in_workspace(|state| state.with_recorded(chords))
    }

    /// The screen after keys were recorded for the action being recorded, or the recording was given up.
    pub fn keys_recorded(self, binding: Option<String>) -> Self {
        self.in_workspace(|state| state.keys_recorded(binding))
    }

    /// The screen with the editor's settings `settings` put to work.
    pub fn with_editor_settings(self, settings: pm_editor::EditorSettings) -> Self {
        self.in_workspace(|state| state.with_editor_settings(settings))
    }

    /// How the editor looks and behaves, while the workspace is showing and its settings kept between runs are in.
    pub fn editor_settings(&self) -> Option<pm_editor::EditorSettings> {
        match self {
            Self::Workspace(state) if state.editor_loaded => Some(state.editor.clone()),
            _ => None,
        }
    }

    /// The screen knowing the view of the file shown is `width` logical pixels wide, with a character of code `cell` wide.
    pub fn with_code_view(self, width: f32, cell: f32) -> Self {
        self.in_workspace(|state| state.with_code_view(width, cell))
    }

    /// How wide the view of the file shown was last drawn and how wide a character of code is in it, while the workspace is showing and it was drawn.
    pub fn code_view(&self) -> Option<(f32, f32)> {
        match self {
            Self::Workspace(state) => state.code_view,
            Self::Onboarding(_) => None,
        }
    }

    /// The screen after the agent of the session at `position` finished a turn while the window watched.
    pub fn agent_finished(self, position: usize) -> Self {
        self.in_workspace(|state| state.agent_finished(position))
    }

    /// The screen after the reader asked to close the window, which asks first while agents are working.
    pub fn close_asked(self) -> Self {
        self.in_workspace(WorkspaceState::close_asked)
    }

    /// Whether the window is to close: the reader asked to and nothing is left to ask first.
    pub fn quitting(&self) -> bool {
        self.workspace_is(|state| state.quitting)
    }

    /// The screen with the window's own settings `settings` put to work.
    pub fn with_window_settings(self, settings: pm_config::WindowSettings) -> Self {
        self.in_workspace(|state| state.with_window_settings(settings))
    }

    /// What the window keeps of its own, while the workspace is showing and those kept between runs are in.
    pub fn window_settings(&self) -> Option<pm_config::WindowSettings> {
        match self {
            Self::Workspace(state) if state.window_loaded => Some(state.window.clone()),
            _ => None,
        }
    }

    /// Whether the workspace is showing without the window's own settings kept between runs put in yet.
    pub fn window_settings_wanted(&self) -> bool {
        matches!(self, Self::Workspace(state) if !state.window_loaded)
    }

    /// The screen knowing the font families installed on this machine: `all` of them, and the `mono` ones among them.
    pub fn with_fonts(self, all: Vec<String>, mono: Vec<String>) -> Self {
        self.in_workspace(|state| state.with_fonts(all, mono))
    }

    /// Whether the workspace is showing without the editor's settings kept between runs put in yet.
    pub fn editor_settings_wanted(&self) -> bool {
        matches!(self, Self::Workspace(state) if !state.editor_loaded)
    }

    /// Whether the file shown has changes that are not saved.
    pub fn shown_dirty(&self) -> bool {
        match self {
            Self::Workspace(state) => state
                .shown_document()
                .is_some_and(|document| document.is_dirty() && !document.field.is_read_only()),
            Self::Onboarding(_) => false,
        }
    }

    /// The screen with the bar for finding in the file shown open, with the row for replacing when `replacing`.
    pub fn find_opened(self, replacing: bool) -> Self {
        self.in_workspace(|state| state.find_opened(replacing))
    }

    /// Whether the bar for finding in the file shown is open.
    pub fn find_open(&self) -> bool {
        self.workspace_is(WorkspaceState::find_open)
    }

    /// The screen with the bar for finding closed.
    pub fn find_closed(self) -> Self {
        self.in_workspace(WorkspaceState::find_closed)
    }

    /// The screen with the next match selected, or the one before when `back`.
    pub fn find_stepped(self, back: bool) -> Self {
        self.in_workspace(|state| state.find_stepped(back))
    }

    /// The screen with every match replaced.
    pub fn replaced_all(self) -> Self {
        self.in_workspace(WorkspaceState::replaced_all)
    }

    /// The screen with the way of matching `which` names switched.
    pub fn find_toggled(self, which: pm_input::FocusId) -> Self {
        self.in_workspace(|state| state.find_toggled(which))
    }

    /// The screen after a file was read as the last commit holds it, or found in none.
    pub fn with_base(self, session: usize, path: &str, base: Option<String>) -> Self {
        self.in_workspace(|state| state.with_base(session, path, base))
    }

    /// The screen asking again for the open files of the session at `session` as the last commit holds them.
    pub fn bases_wanted(self, session: usize) -> Self {
        self.in_workspace(|state| state.bases_wanted(session))
    }

    /// The screen asking the language server where the thing at the caret of the file shown is used.
    pub fn highlights_requested(self) -> Self {
        self.in_workspace(WorkspaceState::highlights_requested)
    }

    /// The screen after the language server said where the thing at a place of a file is used.
    pub fn with_highlights(
        self,
        session: usize,
        path: &str,
        version: i32,
        spans: Vec<pm_protocol::Span>,
    ) -> Self {
        self.in_workspace(|state| state.with_highlights(session, path, version, spans))
    }

    /// The screen after the language server said what the call a place of a file is in looks like.
    pub fn with_signature(
        self,
        session: usize,
        path: &str,
        version: i32,
        signature: Option<pm_protocol::Signature>,
    ) -> Self {
        self.in_workspace(|state| state.with_signature(session, path, version, signature))
    }

    /// The screen asking what the call the caret of the file shown is in looks like.
    pub fn signature_requested(self) -> Self {
        self.in_workspace(WorkspaceState::signature_requested)
    }

    /// Whether the signature of a call shows over the file shown.
    pub fn signature_open(&self) -> bool {
        self.workspace_is(WorkspaceState::signature_open)
    }

    /// The screen with the signature of the call over the file shown taken away.
    pub fn signature_closed(self) -> Self {
        self.in_workspace(WorkspaceState::signature_closed)
    }

    /// The screen noting where the signature of the call the caret is in hangs.
    pub fn with_signature_at(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.with_signature_at(at))
    }

    /// The screen with `what` done to the lines or the selection of the file shown.
    pub fn lines_edited(self, what: pm_components::line_edits::LineEdit) -> Self {
        self.in_workspace(|state| state.lines_edited(what))
    }

    /// Whether the selection of the file shown runs over more than one line.
    pub fn selection_spans_lines(&self) -> bool {
        self.workspace_is(WorkspaceState::selection_spans_lines)
    }

    /// The screen with the box for going to a line by its number open at `at`.
    pub fn go_to_line_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.go_to_line_requested(at))
    }

    /// Whether the box for going to a line is open.
    pub fn going_to_open(&self) -> bool {
        self.workspace_is(WorkspaceState::going_to_open)
    }

    /// The screen with the box for going to a line closed.
    pub fn go_to_line_closed(self) -> Self {
        self.in_workspace(WorkspaceState::go_to_line_closed)
    }

    /// The screen with the selection of the file shown copied without the indentation its lines share.
    pub fn copied_trimmed(self) -> Self {
        self.in_workspace(WorkspaceState::copied_trimmed)
    }

    /// The screen showing the file shown in the file manager.
    pub fn file_revealed(self) -> Self {
        self.in_workspace(WorkspaceState::file_revealed)
    }

    /// The screen with a shell started in the folder of the file shown.
    pub fn terminal_beside_file(self) -> Self {
        self.in_workspace(WorkspaceState::terminal_beside_file)
    }

    /// The screen asking for the web address of the caret's line of the file shown.
    pub fn permalink_requested(self) -> Self {
        self.in_workspace(WorkspaceState::permalink_requested)
    }

    /// The screen opening `address` in the browser.
    pub fn with_address_opened(self, address: String) -> Self {
        self.in_workspace(|state| state.with_address_opened(address))
    }

    /// The screen with the menu on the tab of the open file at `index` open at `at`.
    pub fn with_tab_menu(self, index: usize, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.with_tab_menu(index, at))
    }

    /// The tab of an open file the element `id` is, when it is one.
    pub fn file_tab_of(&self, id: pm_input::FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => {
                id.0.checked_sub(pm_components::open_files::FILE_TAB_BASE)
                    .map(|index| index as usize)
                    .filter(|index| *index < state.git.viewers.len())
            }
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with `text` put on the clipboard.
    pub fn with_copied(self, text: String) -> Self {
        self.in_workspace(|state| state.with_copied(text))
    }

    /// The screen listing the commits that changed the file shown, at `at`.
    pub fn file_history_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.file_history_requested(at))
    }

    /// The screen after the server listed the commits that changed the file at `path` of the session at `session`.
    pub fn with_file_history(
        self,
        session: usize,
        path: &str,
        commits: Vec<pm_git::FileCommit>,
    ) -> Self {
        self.in_workspace(|state| state.with_file_history(session, path, commits))
    }

    /// The screen showing a file of the session at `session` read only, as a commit left it.
    pub fn with_revision(
        self,
        session: usize,
        label: String,
        text: String,
        language: Option<String>,
    ) -> Self {
        self.in_workspace(|state| state.with_revision(session, label, text, language))
    }

    /// The screen with the card about a place taken away.
    pub fn hover_closed(self) -> Self {
        self.in_workspace(WorkspaceState::hover_closed)
    }

    /// Whether a card about a place of the file shown is showing.
    pub fn hover_open(&self) -> bool {
        self.workspace_is(WorkspaceState::hover_open)
    }

    /// The screen with the line broken at the caret of the file shown, indented as its language says.
    pub fn editor_newline(self) -> Self {
        self.in_workspace(WorkspaceState::editor_newline)
    }

    /// The screen with one level of indentation typed at the caret of the file shown.
    pub fn editor_indent(self) -> Self {
        self.in_workspace(WorkspaceState::editor_indent)
    }

    /// The screen with `character` typed into the file shown.
    pub fn editor_typed(self, character: char) -> Self {
        self.in_workspace(|state| state.editor_typed(character))
    }

    /// The screen after the reader chose the folder at `path` to install as an extension.
    pub fn extension_picked(self, path: String) -> Self {
        self.in_workspace(|state| state.extension_picked(path))
    }

    /// The screen after the extensions page is scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_extensions(self, delta: f32, room: f32) -> Self {
        self.in_workspace(|state| state.scrolled_extensions(delta, room))
    }

    /// The screen after a project's overview is scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_overview(self, delta: f32, room: f32) -> Self {
        self.in_workspace(|state| state.scrolled_overview(delta, room))
    }

    /// The screen after the server of `host` said how its languages stand.
    pub fn with_host_extensions(self, host: &Option<String>, languages: &[ExtensionInfo]) -> Self {
        self.in_workspace(|state| state.with_host_extensions(host, languages))
    }

    /// The screen after the server on this machine said which languages there are.
    pub fn with_extensions(self, languages: Vec<ExtensionInfo>) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_extensions(languages)),
            Self::Workspace(state) => Self::Workspace(state.with_extensions(languages)),
        }
    }
}

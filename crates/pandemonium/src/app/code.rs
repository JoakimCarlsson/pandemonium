//! Writing code in the file shown: the keys that indent, complete and ask about a place, where the list of completions hangs, and what the language server is asked when the pointer rests on a word.

use pm_components::areas::WindowEvent;
use pm_components::areas::{MenuEvent, SearchEvent};

use pm_components::areas::TerminalEvent;

use pm_components::areas::SidebarEvent;

use pm_components::areas::SettingsEvent;

use pm_components::areas::EditorEvent;

use pm_components::areas::DiffEvent;

use pm_components::areas::ComposerEvent;

use pm_components::areas::ChatEvent;

use pm_components::areas::BrowserEvent;

use pm_components::events::Event;
use pm_components::target::Target;
use std::time::Duration;

use pm_components::EditorCommand;

use pm_components::line_edits::LineEdit;
use pm_components::listing::{LISTING_HEIGHT, LISTING_WIDTH, QUICK_INSET};

use pm_components::quick_choices::Choosing;
use pm_components::quick_input::{FIELD_HEIGHT, QUICK_SHOWN};

use pm_components::tab_menu::TabCommand;
use pm_components::theme::Appearance;
use pm_components::title_bar::TITLE_BAR_HEIGHT;
use pm_config::ThemeMode;
use pm_editor::Edit;
use pm_editor::{Action, Case, Chord, Keymap, LineOrder, Pressed};
use pm_harness::Verdict;
use pm_pages::Screen;
use pm_protocol::{DebugAction, Seek};
use pm_treesitter::TextObject;
use rustle::{Key, KeyEvent, Modifiers};

use super::App;
use crate::keys;

/// How long a card stays after the pointer left both it and its word, so the pointer can cross the gap between them.
pub(super) const HOVER_GRACE: Duration = Duration::from_millis(350);

/// How many chords a binding recorded on the keymap page takes at most.
const RECORDED_CHORDS: usize = 2;

/// How long recording waits for a second chord before it keeps the first alone.
pub(super) const RECORD_PAUSE: Duration = Duration::from_millis(1000);

/// How long a file waits after it was last changed before it saves itself, when the editor saves by itself.
pub(super) const AUTOSAVE_REST: Duration = Duration::from_millis(1000);

/// How long the caret rests before the language server is asked where the thing at it is used.
pub(super) const HIGHLIGHT_REST: Duration = Duration::from_millis(250);

/// How far below the line it is about the list or card hangs, in logical pixels.
const GAP: f32 = 2.0;

impl App {
    /// Where the lines of code text were drawn in the latest frame, in the
    /// coordinates of the main window's frame.
    pub(super) fn code_lines(&self) -> pm_components::text_lines::TextLines {
        self.model.read(Screen::code_lines)
    }

    /// Where the byte `offset` of the file shown was drawn in the last frame: its left edge, and the top and height of its line.
    fn editor_point(&self, offset: usize) -> Option<(f32, f32, f32)> {
        let line = self
            .code_lines()
            .of(Target::Editor(EditorEvent::EditorField))
            .into_iter()
            .filter(|held| held.start <= offset)
            .max_by_key(|held| held.start)?;
        let caret = line
            .layout
            .caret(offset - line.start, rustle::Affinity::Downstream);
        Some((
            line.rect.origin.x + caret.x,
            line.rect.origin.y,
            line.rect.size.height,
        ))
    }

    /// How tall the window is, in logical pixels.
    pub(super) fn window_height(&self) -> f32 {
        self.window_size().1
    }

    /// Notes where the line of the word the list of completions completes is, as the last
    /// frame drew it: the word's left edge and the line's top, which the list hangs from.
    pub(super) fn place_completion(&mut self) {
        let Some(start) = self.model.read(|state| {
            state
                .shown_document()
                .and_then(|document| document.completion.as_ref())
                .filter(|completion| !completion.loading)
                .map(|completion| completion.start)
        }) else {
            return;
        };
        let Some(at) = self.line_point(start) else {
            return;
        };
        let placed = self.model.read(|state| {
            state
                .shown_document()
                .and_then(|document| document.completion.as_ref())
                .is_some_and(|completion| same_point(completion.at, at))
        });
        if !placed {
            self.model.change(|state| state.with_completion_at(at));
        }
    }

    /// Notes where the line of the caret is while the signature of the call the caret is in
    /// shows, as the last frame drew it: the caret's left edge and the line's top, which the
    /// signature hangs from.
    pub(super) fn place_signature(&mut self) {
        let Some(caret) = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .shown_document()
                .and_then(|document| document.signature.as_ref().map(|_| document.field.caret())),
        }) else {
            return;
        };
        let Some(at) = self.line_point(caret) else {
            return;
        };
        if !self.model.read(|state| same_point(state.signature_at, at)) {
            self.model.change(|state| state.with_signature_at(at));
        }
    }

    /// The left edge of the byte `offset` of the file shown and the top of its line, as the
    /// last frame drew them.
    fn line_point(&self, offset: usize) -> Option<(f32, f32)> {
        self.editor_point(offset).map(|(x, top, _)| (x, top))
    }

    /// The file shown and its version while it has changes to save and the editor saves by
    /// itself: what the root's effect waits on, saving it once it has gone unchanged long
    /// enough.
    pub(super) fn autosave_waiting(&self) -> Option<(String, i32)> {
        self.model.read(|state| {
            let document = state.shown_document()?;
            let dirty = state.editor.autosave && state.shown_dirty();
            dirty.then(|| {
                (
                    state
                        .shown_viewer()
                        .map(|viewer| viewer.path.clone())
                        .unwrap_or_default(),
                    document.version,
                )
            })
        })
    }

    /// Saves the file shown once it went unchanged for [`AUTOSAVE_REST`] as `here`, when it
    /// still waits to be saved as that.
    pub(super) fn autosave_rested(&mut self, here: (String, i32)) {
        if self.autosave_waiting() == Some(here) {
            self.handle(Event::Editor(EditorEvent::EditorSave));
        }
    }

    /// The file, version and caret of the file shown while the code editor has the keyboard
    /// and the language server was not yet asked where the thing there is used: what the
    /// root's effect waits on, asking once the caret rested there long enough.
    pub(super) fn highlight_waiting(&self) -> Option<(String, i32, usize)> {
        if self.focus.focused() != Some(Target::Editor(EditorEvent::EditorField)) {
            return None;
        }
        let here = self.model.read(|state| {
            let path = state.shown_viewer()?.path.clone();
            let document = state.shown_document()?;
            Some((path, document.version, document.field.caret()))
        })?;
        (self.highlighted.as_ref() != Some(&here)).then_some(here)
    }

    /// Asks the language server where the thing at the caret is used, and what else it
    /// tells of the file, once the caret rested at `here` for [`HIGHLIGHT_REST`], when it
    /// still waits there.
    pub(super) fn highlight_rested(&mut self, here: (String, i32, usize)) {
        if self.highlight_waiting().as_ref() != Some(&here) {
            return;
        }
        self.highlighted = Some(here);
        self.model.change(|state| {
            state
                .highlights_requested()
                .annotations_requested()
                .folds_requested()
                .links_requested()
                .blame_requested()
        });
    }

    /// Where the card about the byte `offset` is placed from: the left edge of the byte and the top of its line, as the last frame drew them.
    fn card_at(&self, offset: usize) -> Option<(f32, f32)> {
        let (x, top, _) = self.editor_point(offset)?;
        Some((x, top))
    }

    /// Asks for completions at the caret of the file shown.
    fn complete_at_caret(&mut self) {
        let Some(caret) = self.editor_caret() else {
            return;
        };
        let at = self.line_point(caret).unwrap_or((48.0, 96.0));
        self.model.change(|state| state.completion_requested(at));
    }

    /// Shows what is known about the place at the caret of the file shown.
    fn hover_at_caret(&mut self) {
        let Some(caret) = self.model.read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .map(pm_editor::Field::caret)
        }) else {
            return;
        };
        if let Some(at) = self.card_at(caret) {
            self.model.change(|state| state.hover_requested(caret, at));
        }
    }

    /// Copies the text selected in the card about a place when the keys that copy are pressed, and says whether they were.
    pub(super) fn card_key(&mut self, event: &KeyEvent) -> bool {
        if keys::clip(event) != Some(keys::Clip::Copy) {
            return false;
        }
        let Some(text) = self.model.read(Screen::hover_selection).selected_text() else {
            return false;
        };
        if let Some(clipboard) = self.clipboard() {
            clipboard.set_text(&text);
        }
        true
    }

    /// Whether resting the pointer on a name shows what the language server says about it.
    fn hover_wanted(&self) -> bool {
        self.model.read(|screen| {
            screen
                .editor_settings()
                .is_none_or(|settings| settings.hover)
        })
    }

    /// How long the pointer rests on a word before the language server is asked about it.
    fn hover_rest_time(&self) -> Duration {
        Duration::from_millis(self.model.read(|screen| {
            screen
                .editor_settings()
                .map_or(450, |settings| settings.hover_delay)
        }))
    }

    /// The caret of the file shown.
    fn editor_caret(&self) -> Option<usize> {
        self.model.read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .map(pm_editor::Field::caret)
        })
    }

    /// Where something up to `height` logical pixels tall hangs by the line of the byte `offset` of the file shown: below it, or above it when it would run past the bottom of the window.
    fn hung(&self, offset: usize, height: f32) -> (f32, f32) {
        let Some((x, top, line)) = self.editor_point(offset) else {
            return (48.0, 96.0);
        };
        let below = top + line + GAP;
        if below + height > self.window_height() {
            (x, (top - height - GAP).max(0.0))
        } else {
            (x, below)
        }
    }

    /// Carries out `command` on the file shown, at its caret.
    pub(super) fn editor_command(&mut self, command: EditorCommand) {
        let Some(caret) = self.editor_caret() else {
            return;
        };
        let at = self.hung(caret, LISTING_HEIGHT);
        let seek = match command {
            EditorCommand::Definition => Some(Seek::Definition),
            EditorCommand::Declaration => Some(Seek::Declaration),
            EditorCommand::TypeDefinition => Some(Seek::TypeDefinition),
            EditorCommand::Implementation => Some(Seek::Implementation),
            EditorCommand::References => Some(Seek::References),
            EditorCommand::IncomingCalls => Some(Seek::IncomingCalls),
            EditorCommand::OutgoingCalls => Some(Seek::OutgoingCalls),
            EditorCommand::Supertypes => Some(Seek::Supertypes),
            EditorCommand::Subtypes => Some(Seek::Subtypes),
            _ => None,
        };
        if let Some(seek) = seek {
            self.model
                .change(|state| state.seek_requested(caret, seek, at));
            return;
        }
        self.model.change(|state| match command {
            EditorCommand::Rename => state.rename_requested(caret, self.hung(caret, 72.0)),
            EditorCommand::Format => state.formatted_by_hand(),
            EditorCommand::FormatSelection => state.selection_formatted(),
            EditorCommand::CodeActions => state.code_actions_requested(at),
            EditorCommand::CopyTrimmed => state.copied_trimmed(),
            EditorCommand::Reveal => state.file_revealed(),
            EditorCommand::OpenTerminal => state.terminal_beside_file(),
            EditorCommand::Permalink => state.permalink_requested(),
            EditorCommand::FileHistory => state.file_history_requested(at),
            _ => state,
        });
        if command == EditorCommand::Rename && self.model.read(|screen| screen.renaming_open()) {
            self.focus
                .request_focus(Target::Editor(EditorEvent::RenameSymbol));
        }
    }

    /// The chord a press of keys makes, as the keymap names it.
    pub(super) fn chord_of(event: &KeyEvent) -> Option<Chord> {
        let modifiers = event.modifiers;
        let name = match &event.key {
            Key::Character(_) if keys::is_space(event) => "space".to_owned(),
            Key::Character(_) => event.code_point()?.to_string(),
            Key::Enter => "enter".to_owned(),
            Key::Escape => "escape".to_owned(),
            Key::Tab => "tab".to_owned(),
            Key::Backspace => "backspace".to_owned(),
            Key::Delete => "delete".to_owned(),
            Key::Home => "home".to_owned(),
            Key::End => "end".to_owned(),
            Key::PageUp => "pageup".to_owned(),
            Key::PageDown => "pagedown".to_owned(),
            Key::ArrowUp => "up".to_owned(),
            Key::ArrowDown => "down".to_owned(),
            Key::ArrowLeft => "left".to_owned(),
            Key::ArrowRight => "right".to_owned(),
            Key::F(number @ 1..=12) => format!("f{number}"),
            _ => return None,
        };
        Some(Chord::new(
            &name,
            modifiers.ctrl || modifiers.meta,
            modifiers.alt,
            modifiers.shift,
        ))
    }

    /// The keymap the editor's settings make.
    pub(super) fn keymap(&self) -> Keymap {
        self.model
            .read(|screen| screen.editor_settings().unwrap_or_default().keymap())
    }

    /// Whether `action` may be carried out where the keyboard is now.
    fn may_perform(&self, action: Action) -> bool {
        let focused = self.focus.focused();
        action.everywhere()
            || focused == Some(Target::Editor(EditorEvent::EditorField))
            || (matches!(
                action,
                Action::Find
                    | Action::Replace
                    | Action::FindNext
                    | Action::FindPrevious
                    | Action::ToggleFindInSelection
            ) && (focused == Some(Target::Editor(EditorEvent::FindField))
                || focused == Some(Target::Editor(EditorEvent::ReplaceField))))
    }

    /// Takes a press of keys while keys are recorded for an action on the keymap page: Escape gives up, anything else becomes its keys. Says whether it took the press.
    pub(super) fn recording_key(&mut self, event: &KeyEvent) -> bool {
        if !self.model.read(|screen| screen.recording_keys()) {
            self.recorded.clear();
            return false;
        }
        if event.key == Key::Escape {
            self.recorded.clear();
            self.model.change(|state| state.keys_recorded(None));
            return true;
        }
        let Some(chord) = Self::chord_of(event) else {
            return true;
        };
        self.recorded.push(chord.written());
        if self.recorded.len() >= RECORDED_CHORDS {
            self.finish_recording();
        } else {
            let so_far = self.recorded.clone();
            self.model.change(|state| state.with_recorded(so_far));
        }
        true
    }

    /// Keeps the chords recorded so far as the action's keys.
    fn finish_recording(&mut self) {
        let binding = std::mem::take(&mut self.recorded).join(" ");
        self.model
            .change(|state| state.keys_recorded((!binding.is_empty()).then_some(binding)));
        self.persist();
    }

    /// The chords recorded so far while recording waits for another: what the root's effect
    /// waits on, keeping them once no further chord came for [`RECORD_PAUSE`].
    pub(super) fn recording_waiting(&self) -> Option<Vec<String>> {
        (!self.recorded.is_empty()).then(|| self.recorded.clone())
    }

    /// Keeps the chords `recorded` as the action's keys once no further chord came in time,
    /// when they are still what was recorded.
    pub(super) fn recording_rested(&mut self, recorded: Vec<String>) {
        if self.recorded == recorded {
            self.finish_recording();
        }
    }

    /// Takes a press of keys the keymap binds to an action, or that starts a binding of more chords, and says whether it took it; typing goes on as typing.
    pub(super) fn keymap_key(&mut self, event: &KeyEvent) -> bool {
        let modifiers = event.modifiers;
        let typing = matches!(event.key, Key::Character(_))
            && !(modifiers.ctrl || modifiers.meta || modifiers.alt);
        if typing && self.pending.is_empty() {
            return false;
        }
        let Some(chord) = Self::chord_of(event) else {
            return false;
        };
        match self.keymap().press(&self.pending, &chord) {
            Pressed::Waiting => {
                self.pending.push(chord);
                true
            }
            Pressed::Action(action) => {
                self.pending.clear();
                if !self.may_perform(action) {
                    return false;
                }
                self.perform(action);
                true
            }
            Pressed::Nothing => !std::mem::take(&mut self.pending).is_empty(),
        }
    }

    /// Where a list opened from anywhere hangs: over the command center in the middle of the title bar, its field lying on the command center.
    fn across_top(&self) -> (f32, f32) {
        let width = self.window_size().0;
        let center = self.place(Target::Window(WindowEvent::CommandCenter));
        let (middle, field_top) = center.map_or(
            (width / 2.0, (TITLE_BAR_HEIGHT - FIELD_HEIGHT) / 2.0),
            |rect| (rect.origin.x + rect.size.width / 2.0, rect.origin.y),
        );
        let left = (middle - LISTING_WIDTH / 2.0).clamp(0.0, (width - LISTING_WIDTH).max(0.0));
        (left, (field_top - QUICK_INSET).max(0.0))
    }

    /// Carries out the command the list takes when `id`, one of its rows or the key that takes the row picked, is pressed, and says whether the list held commands.
    pub(super) fn take_command(&mut self, id: Target) -> bool {
        let row = match id {
            Target::Editor(EditorEvent::ListingRow(row)) => Some(row),
            _ => None,
        };
        if row.is_none() && id != Target::Editor(EditorEvent::ListingTake) {
            return false;
        }
        let Some(action) = self.model.read(|screen| screen.command_taken(row)) else {
            let Some((host, command)) = self
                .model
                .read(|screen| screen.extension_command_taken(row))
            else {
                return false;
            };
            self.model
                .change(|state| state.listing_closed().extension_command_run(host, command));
            self.leave_listing();
            self.deliver();
            return true;
        };
        self.model
            .change(|state| state.listing_closed().command_used(action));
        self.leave_listing();
        self.perform(action);
        true
    }

    /// Gives the keyboard back to the file shown once the list over it closed, or lets go of it when no file is shown.
    fn leave_listing(&mut self) {
        if self.model.read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .is_some()
        }) {
            self.focus
                .request_focus(Target::Editor(EditorEvent::EditorField));
        } else {
            self.focus.free_focus();
        }
    }

    /// Opens the quick input over the choices of `choosing`.
    fn choose(&mut self, choosing: Choosing) {
        let at = self.across_top();
        self.model
            .change(|state| state.choices_requested(choosing, at));
    }

    /// Presses the row `row` of the menu of the session shown.
    fn session_command(&mut self, row: Target) {
        self.model.change(|state| state.session_command(row));
        self.deliver();
    }

    /// Carries out `command` on the tab shown, as its menu does.
    fn tab_command(&mut self, command: TabCommand) {
        self.model.change(|state| state.shown_tab_command(command));
        self.deliver();
    }

    /// Carries out `action`.
    pub(super) fn perform(&mut self, action: Action) {
        let command = match action {
            Action::GoToDefinition => Some(EditorCommand::Definition),
            Action::GoToDeclaration => Some(EditorCommand::Declaration),
            Action::GoToTypeDefinition => Some(EditorCommand::TypeDefinition),
            Action::GoToImplementation => Some(EditorCommand::Implementation),
            Action::FindReferences => Some(EditorCommand::References),
            Action::IncomingCalls => Some(EditorCommand::IncomingCalls),
            Action::Rename => Some(EditorCommand::Rename),
            Action::Format => Some(EditorCommand::Format),
            Action::FormatSelection => Some(EditorCommand::FormatSelection),
            Action::CodeActions => Some(EditorCommand::CodeActions),
            Action::RevealInFileManager => Some(EditorCommand::Reveal),
            _ => None,
        };
        if let Some(command) = command {
            self.editor_command(command);
            return;
        }
        let edit = match action {
            Action::ToggleComment => Some(LineEdit::ToggleComment),
            Action::Indent => Some(LineEdit::Indent),
            Action::Outdent => Some(LineEdit::Outdent),
            Action::MoveLineUp => Some(LineEdit::MoveUp),
            Action::MoveLineDown => Some(LineEdit::MoveDown),
            Action::DuplicateLineUp => Some(LineEdit::DuplicateUp),
            Action::DuplicateLineDown => Some(LineEdit::DuplicateDown),
            Action::DeleteLine => Some(LineEdit::Delete),
            Action::ExpandSelection => Some(LineEdit::Grow),
            Action::ShrinkSelection => Some(LineEdit::Shrink),
            Action::SelectFunction => Some(LineEdit::SelectObject(TextObject::Function, false)),
            Action::SelectFunctionBody => Some(LineEdit::SelectObject(TextObject::Function, true)),
            Action::SelectClass => Some(LineEdit::SelectObject(TextObject::Class, false)),
            Action::SelectClassBody => Some(LineEdit::SelectObject(TextObject::Class, true)),
            Action::SelectComment => Some(LineEdit::SelectObject(TextObject::Comment, false)),
            Action::NextProblem => Some(LineEdit::NextProblem),
            Action::NextChange => Some(LineEdit::NextChange),
            Action::PreviousChange => Some(LineEdit::PreviousChange),
            Action::PreviousProblem => Some(LineEdit::PreviousProblem),
            Action::AddNextOccurrence => Some(LineEdit::AddNextOccurrence),
            Action::SelectAllOccurrences => Some(LineEdit::SelectAllOccurrences),
            Action::AddCaretAbove => Some(LineEdit::AddCaretAbove),
            Action::AddCaretBelow => Some(LineEdit::AddCaretBelow),
            Action::SelectLine => Some(LineEdit::SelectLine),
            Action::JoinLines => Some(LineEdit::JoinLines),
            Action::UppercaseSelection => Some(LineEdit::Case(Case::Upper)),
            Action::LowercaseSelection => Some(LineEdit::Case(Case::Lower)),
            Action::TitleCaseSelection => Some(LineEdit::Case(Case::Title)),
            Action::SwapCaseSelection => Some(LineEdit::Case(Case::Swap)),
            Action::SnakeCaseSelection => Some(LineEdit::Case(Case::Snake)),
            Action::KebabCaseSelection => Some(LineEdit::Case(Case::Kebab)),
            Action::CamelCaseSelection => Some(LineEdit::Case(Case::Camel)),
            Action::PascalCaseSelection => Some(LineEdit::Case(Case::Pascal)),
            Action::ConstantCaseSelection => Some(LineEdit::Case(Case::Constant)),
            Action::SortLinesAscending => Some(LineEdit::OrderLines(LineOrder::Ascending)),
            Action::SortLinesDescending => Some(LineEdit::OrderLines(LineOrder::Descending)),
            Action::SortLinesIgnoringCase => Some(LineEdit::OrderLines(LineOrder::IgnoringCase)),
            Action::ReverseLines => Some(LineEdit::OrderLines(LineOrder::Reversed)),
            Action::UniqueLines => Some(LineEdit::OrderLines(LineOrder::Unique)),
            Action::TransposeCharacters => Some(LineEdit::Transpose),
            Action::TrimTrailingWhitespace => Some(LineEdit::TrimTrailing),
            Action::DeleteToLineEnd => Some(LineEdit::DeleteToLineEnd),
            Action::DeleteToLineStart => Some(LineEdit::DeleteToLineStart),
            Action::ToggleBlockComment => Some(LineEdit::ToggleBlockComment),
            Action::SplitSelectionIntoLines => Some(LineEdit::SplitIntoLines),
            Action::ColumnSelectUp => Some(LineEdit::Column(-1, 0)),
            Action::ColumnSelectDown => Some(LineEdit::Column(1, 0)),
            Action::ColumnSelectLeft => Some(LineEdit::Column(0, -1)),
            Action::ColumnSelectRight => Some(LineEdit::Column(0, 1)),
            Action::MatchingBracket => Some(LineEdit::MatchingBracket),
            Action::NewlineBelow => Some(LineEdit::LineBelow),
            Action::NewlineAbove => Some(LineEdit::LineAbove),
            Action::Fold => Some(LineEdit::Fold),
            Action::Unfold => Some(LineEdit::Unfold),
            Action::FoldAll => Some(LineEdit::FoldAll),
            Action::CursorUndo => Some(LineEdit::CursorUndo),
            Action::CursorRedo => Some(LineEdit::CursorRedo),
            Action::UnfoldAll => Some(LineEdit::UnfoldAll),
            _ => None,
        };
        if let Some(what) = edit {
            self.model.change(|state| state.lines_edited(what));
            self.show_editor_caret();
            return;
        }
        match action {
            Action::Hover => self.hover_at_caret(),
            Action::Complete => self.complete_at_caret(),
            Action::OrganizeImports | Action::FixAll => {
                let at = self
                    .editor_caret()
                    .map_or((48.0, 96.0), |caret| self.hung(caret, LISTING_HEIGHT));
                self.model.change(|state| {
                    if action == Action::FixAll {
                        state.fix_all_requested(at)
                    } else {
                        state.organize_imports_requested(at)
                    }
                });
                self.deliver();
            }
            Action::SignatureHelp => self.model.change(|state| state.signature_requested()),
            Action::GoBack => self.navigate(-1),
            Action::GoForward => self.navigate(1),
            Action::Find | Action::Replace => {
                let replacing = action == Action::Replace;
                self.model.change(|state| state.find_opened(replacing));
                self.focus
                    .request_focus(Target::Editor(EditorEvent::FindField));
                self.show_editor_caret();
            }
            Action::ToggleFindInSelection => {
                self.model.change(|state| state.find_scope_toggled());
                self.focus
                    .request_focus(Target::Editor(EditorEvent::FindField));
                self.show_editor_caret();
            }
            Action::FindNext | Action::FindPrevious => {
                let back = action == Action::FindPrevious;
                self.model.change(|state| state.find_stepped(back));
                self.show_editor_caret();
            }
            Action::GoToLine => {
                let at = self
                    .editor_caret()
                    .map_or((48.0, 96.0), |caret| self.hung(caret, 72.0));
                self.model.change(|state| state.go_to_line_requested(at));
                self.focus
                    .request_focus(Target::Editor(EditorEvent::GoToLine));
            }
            Action::Outline => {
                let at = self.across_top();
                self.model.change(|state| state.outline_requested(at));
            }
            Action::ProjectSymbols => {
                let at = self.across_top();
                self.model.change(|state| state.symbols_requested(at));
            }
            Action::FileFinder => {
                let at = self.across_top();
                self.model.change(|state| state.file_finder_requested(at));
            }
            Action::SearchProject => {
                self.model.change(|state| state.project_search_opened());
                self.focus
                    .request_focus(Target::Search(SearchEvent::ProjectSearchField));
            }
            Action::Problems => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).problems_shown());
            }
            Action::ToggleBreakpoint => {
                self.model.update(|screen| {
                    *screen = std::mem::take(screen).breakpoint_toggled_at_caret()
                });
                self.deliver();
            }
            Action::StartDebugging => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).debug_started(at));
                self.deliver();
            }
            Action::AttachDebugger => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).debug_attach_requested(at));
                self.deliver();
            }
            Action::RunTask => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).tasks_requested(at));
                self.deliver();
            }
            Action::RerunTask => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).task_rerun());
                self.deliver();
            }
            Action::SelectToolchain => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).toolchains_requested(at));
                self.deliver();
            }
            Action::StopDebugging
            | Action::PauseDebugging
            | Action::StepOver
            | Action::StepInto
            | Action::StepOut => {
                let what = match action {
                    Action::StopDebugging => DebugAction::Stop,
                    Action::PauseDebugging => DebugAction::Pause,
                    Action::StepOver => DebugAction::StepOver,
                    Action::StepInto => DebugAction::StepIn,
                    _ => DebugAction::StepOut,
                };
                self.model
                    .update(|screen| *screen = std::mem::take(screen).debug_do(what));
                self.deliver();
            }
            Action::Save => self.handle(Event::Editor(EditorEvent::EditorSave)),
            Action::CompareWithClipboard => {
                let text = self
                    .clipboard()
                    .and_then(|clipboard| clipboard.text())
                    .unwrap_or_default();
                self.model
                    .change(|state| state.compare_with_clipboard(text));
            }
            Action::CompareWithSaved => {
                self.model
                    .change(|state| state.compare_with_saved_requested());
                self.deliver();
            }
            Action::CompareWithFile => {
                let at = self.across_top();
                self.model
                    .change(|state| state.compare_with_file_requested(at));
            }
            Action::CommandPalette => {
                let at = self.across_top();
                self.model.change(|state| state.commands_requested(at));
            }
            Action::OpenSettings => self.open_settings(),
            Action::GoToSession => {
                let at = self.across_top();
                self.model.change(|state| state.sessions_requested(at));
            }
            Action::NewSession => {
                if let Some(id) = self.model.read(Screen::new_session_button) {
                    self.handle(Event::pressing(id));
                }
            }
            Action::NewChat => self.handle(Event::Sidebar(SidebarEvent::NewChat)),
            Action::AddProject => self.handle(Event::Sidebar(SidebarEvent::AddProject)),
            Action::ShowHistory => self.handle(Event::Sidebar(SidebarEvent::History)),
            Action::ShowExtensions => self.handle(Event::Settings(SettingsEvent::Extensions)),
            Action::WorkspaceBack => self.navigate(-1),
            Action::WorkspaceForward => self.navigate(1),
            Action::Quit => self.closing = self.close_window(),
            Action::ShowChat => self.handle(Event::Chat(ChatEvent::TabChat)),
            Action::ShowDiff => self.handle(Event::Diff(DiffEvent::TabDiff)),
            Action::StopAgent => self.handle(Event::Composer(ComposerEvent::Stop)),
            Action::ToggleTerminal => self.handle(Event::Terminal(TerminalEvent::Terminal)),
            Action::NewTerminal => self.handle(Event::Terminal(TerminalEvent::NewTerminal)),
            Action::OpenBrowser => self.open_browser(),
            Action::ToggleBrowserTools => {
                self.handle(Event::Browser(BrowserEvent::BrowserTools));
            }
            Action::ToggleSourceControl => self.handle(Event::Diff(DiffEvent::Review)),
            Action::ChangeModel | Action::ChangeMode => {
                let category = if action == Action::ChangeModel {
                    "model"
                } else {
                    "mode"
                };
                let at = self.across_top();
                self.model
                    .change(|state| state.agent_category_requested(category, at));
            }
            Action::ChangeAgentSetting => self.choose(Choosing::AgentSetting),
            Action::ChangeTheme => self.choose(Choosing::Theme),
            Action::CheckoutBranch | Action::CreateBranch => self.choose(Choosing::Branch),
            Action::ChangeKeymap => self.choose(Choosing::Keymap),
            Action::RewindConversation => self.choose(Choosing::Rewind),
            Action::ForkConversation => self.choose(Choosing::Fork),
            Action::RestoreFiles => self.choose(Choosing::Restore),
            Action::RetryReply => {
                self.model.change(|state| state.retry_requested());
                self.deliver();
            }
            Action::StartFresh => {
                self.model.change(|state| state.fresh_requested());
                self.deliver();
            }
            Action::HandOffConversation => {
                self.model.change(|state| state.handoff_requested());
                self.focus
                    .request_focus(Target::Composer(ComposerEvent::Prompt));
            }
            Action::AskOnTheSide => {
                self.model.change(|state| state.btw_requested());
                self.focus
                    .request_focus(Target::Composer(ComposerEvent::Prompt));
            }
            Action::CycleMode => self.model.change(|state| state.mode_cycled()),
            Action::AllowRequest | Action::AlwaysAllowRequest | Action::RejectRequest => {
                let verdict = match action {
                    Action::AllowRequest => Verdict::Allow,
                    Action::AlwaysAllowRequest => Verdict::Always,
                    _ => Verdict::Reject,
                };
                if let Some(id) = self.model.read(|screen| screen.answer_of(verdict)) {
                    self.handle(Event::pressing(id));
                }
            }
            Action::RenameSession => {
                self.session_command(Target::Menu(MenuEvent::SessionMenuRename))
            }
            Action::ArchiveSession => {
                self.session_command(Target::Menu(MenuEvent::SessionMenuClose))
            }
            Action::CopySessionPath => {
                self.session_command(Target::Menu(MenuEvent::SessionMenuCopyPath))
            }
            Action::CopyBranchName => {
                self.session_command(Target::Menu(MenuEvent::SessionMenuCopyBranch))
            }
            Action::OpenSessionFolder => {
                self.session_command(Target::Menu(MenuEvent::SessionMenuOpenFolder))
            }
            Action::FocusPrompt => self
                .focus
                .request_focus(Target::Composer(ComposerEvent::Prompt)),
            Action::NextTab | Action::PreviousTab => {
                let delta = if action == Action::NextTab { 1 } else { -1 };
                self.model.change(|state| state.tab_stepped(delta));
            }
            Action::ReopenTab => {
                self.model.change(|state| state.tab_reopened());
                self.deliver();
            }
            Action::CloseTab => self.tab_command(TabCommand::Close),
            Action::CloseOtherTabs => self.tab_command(TabCommand::CloseOthers),
            Action::CloseSavedTabs => self.tab_command(TabCommand::CloseClean),
            Action::CloseAllTabs => self.tab_command(TabCommand::CloseAll),
            Action::PinTab => self.tab_command(TabCommand::TogglePin),
            Action::ToggleReadOnly => self.tab_command(TabCommand::ToggleReadOnly),
            Action::CopyPath => self.tab_command(TabCommand::CopyPath),
            Action::CopyRelativePath => self.tab_command(TabCommand::CopyRelativePath),
            Action::RevealInProject => self.tab_command(TabCommand::RevealInProject),
            Action::OpenPermalink => self.tab_command(TabCommand::OpenPermalink),
            Action::SplitRight => self.handle(Event::Window(WindowEvent::SplitRight)),
            Action::SplitLeft => self.handle(Event::Window(WindowEvent::SplitLeft)),
            Action::SplitUp => self.handle(Event::Window(WindowEvent::SplitUp)),
            Action::SplitDown => self.handle(Event::Window(WindowEvent::SplitDown)),
            Action::FocusNextPane | Action::FocusPreviousPane => {
                let delta = if action == Action::FocusNextPane {
                    1
                } else {
                    -1
                };
                self.model.change(|state| state.pane_stepped(delta));
            }
            Action::ToggleTheme => {
                let mode = if self.appearance == Appearance::Dark {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
                self.model.change(|state| state.with_theme_mode(mode));
            }
            Action::ZoomIn | Action::ZoomOut | Action::ResetZoom => {
                let steps = match action {
                    Action::ZoomIn => Some(1),
                    Action::ZoomOut => Some(-1),
                    _ => None,
                };
                self.model.change(|state| state.zoomed(steps));
            }
            Action::CopyPermalink => self.editor_command(EditorCommand::Permalink),
            Action::FileHistory => self.editor_command(EditorCommand::FileHistory),
            Action::OpenTerminalHere => self.editor_command(EditorCommand::OpenTerminal),
            Action::OutgoingCalls => self.editor_command(EditorCommand::OutgoingCalls),
            Action::ShowSupertypes => self.editor_command(EditorCommand::Supertypes),
            Action::ShowSubtypes => self.editor_command(EditorCommand::Subtypes),
            Action::CopyWithoutIndentation => self.editor_command(EditorCommand::CopyTrimmed),
            Action::Undo => self.edit_field(Target::Editor(EditorEvent::EditorField), Edit::Undo),
            Action::Redo => self.edit_field(Target::Editor(EditorEvent::EditorField), Edit::Redo),
            Action::Commit
            | Action::CommitAndPush
            | Action::Pull
            | Action::Push
            | Action::Fetch
            | Action::StashChanges
            | Action::PullRebase
            | Action::ForcePush
            | Action::Sync
            | Action::StageAll
            | Action::UnstageAll
            | Action::CommitAmend
            | Action::ContinueOperation
            | Action::SkipCommit
            | Action::AbortOperation
            | Action::StashApply
            | Action::StashPop
            | Action::StashDrop
            | Action::ToggleGraph
            | Action::CloneRepository
            | Action::RefreshSourceControl => {
                self.model.change(|state| state.source_control(action));
                self.deliver();
            }
            _ => {}
        }
    }

    /// Takes a key pressed while the box for a new name or for a line to go to has the keyboard: Escape closes it and gives the keyboard back to the file. Says whether it took the key.
    pub(super) fn rename_key(&mut self, event: &KeyEvent) -> bool {
        let focused = self.focus.focused();
        if event.key != Key::Escape
            || !(focused == Some(Target::Editor(EditorEvent::RenameSymbol))
                || focused == Some(Target::Editor(EditorEvent::GoToLine))
                || focused == Some(Target::Editor(EditorEvent::ColorHex)))
        {
            return false;
        }
        self.model.change(|state| {
            state
                .rename_closed()
                .go_to_line_closed()
                .color_picker_closed()
        });
        self.focus
            .request_focus(Target::Editor(EditorEvent::EditorField));
        true
    }

    /// Takes a key pressed while the field narrowing the list over the file has the keyboard: the arrows move the pick and Escape closes the list. Says whether it took the key.
    pub(super) fn listing_key(&mut self, event: &KeyEvent) -> bool {
        let focused = self.focus.focused();
        let held = focused == Some(Target::Editor(EditorEvent::ListingFilter)) || focused.is_none();
        if !held || !self.model.read(|screen| screen.listing_open()) {
            return false;
        }
        let page = QUICK_SHOWN as isize - 1;
        let delta = match event.key {
            Key::ArrowUp => -1,
            Key::ArrowDown => 1,
            Key::PageUp => -page,
            Key::PageDown => page,
            Key::Escape => {
                self.model.change(|state| state.listing_closed());
                self.leave_listing();
                return true;
            }
            _ => return false,
        };
        self.model.change(|state| state.listing_moved(delta));
        true
    }

    /// Takes a key pressed while the bar for finding has the keyboard: Escape closes it, Shift+Enter goes back a match, Ctrl+Alt+Enter replaces every match, and Alt with C, W or R switches matching case, whole words or regular expressions. Says whether it took the key.
    pub(super) fn find_key(&mut self, event: &KeyEvent) -> bool {
        let focused = self.focus.focused();
        if focused != Some(Target::Editor(EditorEvent::FindField))
            && focused != Some(Target::Editor(EditorEvent::ReplaceField))
        {
            return false;
        }
        let modifiers = event.modifiers;
        let command = modifiers.ctrl || modifiers.meta;
        let toggled = match keys::letter(event) {
            Some('c') if modifiers.alt => Some(Target::Editor(EditorEvent::FindCase)),
            Some('w') if modifiers.alt => Some(Target::Editor(EditorEvent::FindWord)),
            Some('r') if modifiers.alt => Some(Target::Editor(EditorEvent::FindRegex)),
            _ => None,
        };
        if let Some(which) = toggled {
            self.model.change(|state| state.find_toggled(which));
            self.show_editor_caret();
            return true;
        }
        match event.key {
            Key::Escape => {
                self.model.change(|state| state.find_closed());
                self.focus
                    .request_focus(Target::Editor(EditorEvent::EditorField));
            }
            Key::Enter
                if focused == Some(Target::Editor(EditorEvent::ReplaceField))
                    && command
                    && modifiers.alt =>
            {
                self.model.change(|state| state.replaced_all());
            }
            Key::Enter
                if focused == Some(Target::Editor(EditorEvent::FindField)) && modifiers.shift =>
            {
                self.model.change(|state| state.find_stepped(true));
            }
            _ => return false,
        }
        self.show_editor_caret();
        true
    }

    /// What the keys of `event` that work as in any editor do to the lines or the selection of the file shown, when they do something, with whether the selection runs over lines, `spanning`: Home, Tab and Shift+Tab, and Backspace.
    fn line_edit_of(event: &KeyEvent, spanning: bool) -> Option<LineEdit> {
        let modifiers = event.modifiers;
        let command = modifiers.ctrl || modifiers.meta;
        let (alt, shift) = (modifiers.alt, modifiers.shift);
        match event.key {
            Key::Home if !command && !alt => Some(LineEdit::Home(shift)),
            Key::Tab if shift && !command => Some(LineEdit::Outdent),
            Key::Tab if spanning && !command => Some(LineEdit::Indent),
            Key::Backspace if !command && !alt && !shift => Some(LineEdit::Backspace),
            _ => None,
        }
    }

    /// Gives the keyboard back to the file shown once the box for a new name or a line to go to it was lent to has closed.
    pub(super) fn settle_rename_focus(&mut self) {
        let focused = self.focus.focused();
        let closed = (focused == Some(Target::Editor(EditorEvent::RenameSymbol))
            && !self.model.read(|screen| screen.renaming_open()))
            || (focused == Some(Target::Editor(EditorEvent::GoToLine))
                && !self.model.read(|screen| screen.going_to_open()))
            || (focused == Some(Target::Editor(EditorEvent::ColorHex))
                && !self.model.read(|screen| screen.color_picker_open()))
            || ((focused == Some(Target::Editor(EditorEvent::FindField))
                || focused == Some(Target::Editor(EditorEvent::ReplaceField)))
                && !self.model.read(|screen| screen.find_open()))
            || (focused == Some(Target::Editor(EditorEvent::ListingFilter))
                && !self.model.read(|screen| screen.listing_open()));
        if closed {
            self.focus
                .request_focus(Target::Editor(EditorEvent::EditorField));
            self.show_editor_caret();
        }
    }

    /// Takes a key pressed while the file shown has the keyboard, when it is one of the keys for writing code, and says whether it was.
    pub(super) fn editor_key(&mut self, event: &KeyEvent) -> bool {
        if self.focus.focused() != Some(Target::Editor(EditorEvent::EditorField)) {
            return false;
        }
        let modifiers = event.modifiers;
        let command = modifiers.ctrl || modifiers.meta;
        let spanning = self.model.read(|screen| screen.selection_spans_lines());
        if event.key == Key::Tab
            && modifiers.shift
            && !command
            && self.model.read(|screen| screen.snippet_active())
        {
            self.model.change(|state| state.snippet_back());
            self.show_editor_caret();
            return true;
        }
        if let Some(what) = Self::line_edit_of(event, spanning) {
            self.model.change(|state| state.lines_edited(what));
            self.show_editor_caret();
            return true;
        }
        let (completing, hovering, listing, signing, finding, snippeting) =
            self.model.read(|screen| {
                (
                    screen.completion_open(),
                    screen.hover_open(),
                    screen.listing_open(),
                    screen.signature_open(),
                    screen.find_open(),
                    screen.snippet_active(),
                )
            });
        let several = self.model.read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .is_some_and(pm_editor::Field::has_others)
        });
        if listing {
            let changed = |screen: Screen| -> Option<Screen> {
                match event.key {
                    Key::ArrowUp => Some(screen.in_workspace(|state| state.listing_moved(-1))),
                    Key::ArrowDown => Some(screen.in_workspace(|state| state.listing_moved(1))),
                    Key::Enter => Some(screen.in_workspace(|state| state.listing_taken(None))),
                    Key::Escape => Some(screen.in_workspace(|state| state.listing_closed())),
                    _ => None,
                }
            };
            let mut taken = false;
            self.model.update(|screen| {
                if let Some(next) = changed(screen.clone()) {
                    *screen = next;
                    taken = true;
                }
            });
            if taken {
                self.show_editor_caret();
                return true;
            }
            self.model.change(|state| state.listing_closed());
        }
        let next = |screen: Screen| -> Option<Screen> {
            match event.key {
                Key::ArrowUp if completing => {
                    Some(screen.in_workspace(|state| state.completion_moved(-1)))
                }
                Key::ArrowDown if completing => {
                    Some(screen.in_workspace(|state| state.completion_moved(1)))
                }
                Key::Enter | Key::Tab if completing && !modifiers.shift => {
                    Some(screen.in_workspace(|state| state.completion_taken(None)))
                }
                Key::Escape if completing => {
                    Some(screen.in_workspace(|state| state.completion_closed()))
                }
                Key::Escape if signing => {
                    Some(screen.in_workspace(|state| state.signature_closed()))
                }
                Key::Escape if snippeting => {
                    Some(screen.in_workspace(|state| state.snippet_ended()))
                }
                Key::Escape if finding => Some(screen.in_workspace(|state| state.find_closed())),
                Key::Escape if several => {
                    Some(screen.in_workspace(|state| state.lines_edited(LineEdit::OnlyCaret)))
                }
                Key::Escape if hovering => Some(screen.in_workspace(|state| state.hover_closed())),
                Key::Enter if !command && !modifiers.shift && !modifiers.alt => {
                    Some(screen.in_workspace(|state| state.editor_newline()))
                }
                Key::Tab if !modifiers.shift && !command => {
                    Some(screen.in_workspace(|state| state.editor_tab()))
                }
                _ => match keys::edit(event) {
                    Some(Edit::Insert(character)) if !command => {
                        Some(screen.in_workspace(|state| state.editor_typed(character)))
                    }
                    _ => None,
                },
            }
        };
        if hovering && event.key != Key::Escape {
            self.model.change(|state| state.hover_closed());
        }
        let mut taken = false;
        self.model.update(|screen| {
            if let Some(changed) = next(screen.clone()) {
                *screen = changed;
                taken = true;
            }
        });
        if taken {
            self.show_editor_caret();
        }
        taken
    }

    /// Notes that the pointer is over the drawn character at `byte` of the file shown, or
    /// over none, as the code tells, so the language server is asked about the word there
    /// once it has rested long enough. A card about a word stays while the pointer is on the
    /// word or on the card; it is taken away a moment after the pointer left the word, so the
    /// pointer can cross to the card, and at once when the pointer leaves the card itself.
    pub(super) fn code_hovered(&mut self, byte: Option<usize>) {
        self.editor_hover = byte;
        let selecting = self.hover_card_held
            && self
                .model
                .read(Screen::hover_selection)
                .selection()
                .is_some();
        if self.hover_card_over || selecting {
            self.hover_rest = None;
            self.hover_leaving = false;
            return;
        }
        let stretch = self.word_at(byte);
        self.reach(self.reach_at(byte).filter(|_| self.modifiers.ctrl));
        let word = stretch.map(|stretch| stretch.start);
        let shown = self.model.read(|state| {
            state
                .shown_document()
                .and_then(|document| document.hover.as_ref())
                .map(|card| card.offset)
        });
        let left = shown.is_some_and(|offset| word != Some(offset));
        if left && std::mem::take(&mut self.hover_entered) {
            self.hover_leaving = false;
            self.model.change(|state| state.hover_closed());
        } else {
            self.hover_leaving = left;
        }
        if shown.is_none() {
            self.hover_entered = false;
        }
        let word = word.filter(|_| self.hover_wanted());
        self.hover_rest = word.filter(|start| Some(*start) != shown);
    }

    /// Notes that the pointer came over the card about a word of the file shown, which keeps
    /// it showing, or left it when not `over`, which counts as the pointer being where the
    /// code last said it is.
    pub(super) fn hover_card_hovered(&mut self, over: bool) {
        self.hover_card_over = over;
        if over {
            self.hover_rest = None;
            self.hover_leaving = false;
            self.hover_entered = true;
        } else {
            self.code_hovered(self.editor_hover);
        }
    }

    /// Notes that the primary button went down on the card about a word, to select its
    /// text, or came up when not `held`.
    pub(super) fn hover_card_held(&mut self, held: bool) {
        self.hover_card_held = held;
    }

    /// The link of the file shown whose drawn character at `byte` the pointer is over, when
    /// it is on one: the bytes it covers and where it leads.
    fn link_at(
        &self,
        byte: Option<usize>,
    ) -> Option<(std::ops::Range<usize>, pm_protocol::LinkTarget)> {
        self.model.read(|screen| screen.link_under(byte?))
    }

    /// What Ctrl underlines at the drawn character at `byte`: the link there, or else the
    /// word, which is what a press takes to its definition.
    fn reach_at(&self, byte: Option<usize>) -> Option<std::ops::Range<usize>> {
        self.link_at(byte)
            .map(|(range, _)| range)
            .or_else(|| self.word_at(byte))
    }

    /// The word of the file shown whose drawn character at `byte` the pointer is over, when
    /// it is on one.
    fn word_at(&self, byte: Option<usize>) -> Option<std::ops::Range<usize>> {
        let byte = byte?;
        self.model.read(|screen| {
            let field = screen.field(Target::Editor(EditorEvent::EditorField))?;
            let text = field.text();
            let word = field.word_range_at(byte);
            text[word.clone()]
                .chars()
                .any(|character| character.is_alphanumeric() || character == '_')
                .then_some(word)
        })
    }

    /// Opens the box for changing the colour at `index` of the colours of the file shown, hanging by the line it is on, and gives it the keyboard.
    pub(super) fn open_color_picker(&mut self, index: usize) {
        let Some(offset) = self.model.read(|screen| screen.color_swatch_offset(index)) else {
            return;
        };
        let at = self.hung(offset, 150.0);
        self.model
            .change(|state| state.color_picker_requested(index, at));
        self.deliver();
        if self.model.read(|screen| screen.color_picker_open()) {
            self.focus
                .request_focus(Target::Editor(EditorEvent::ColorHex));
        }
    }

    /// Underlines `stretch` of the file shown as what a press takes to its definition, or nothing, when that differs from what is underlined.
    fn reach(&mut self, stretch: Option<std::ops::Range<usize>>) {
        let held = self.model.read(|state| {
            state
                .shown_document()
                .and_then(|document| document.reach.clone())
        });
        if held != stretch {
            self.model.change(|state| state.with_reach(stretch));
        }
    }

    /// The drawn character of the file shown the pointer is over, as the code last told,
    /// unless the card about a word is over it.
    fn editor_hovered(&self) -> Option<usize> {
        self.editor_hover.filter(|_| !self.hover_card_over)
    }

    /// Underlines the word under the pointer while Ctrl is held, and takes the underline away when it is let go.
    pub(super) fn editor_modifiers_changed(&mut self) {
        let stretch = self
            .modifiers
            .ctrl
            .then(|| self.reach_at(self.editor_hovered()))
            .flatten();
        self.reach(stretch);
    }

    /// Takes the reader to where the word under the pointer is defined, when Ctrl is held in
    /// `modifiers` as the file shown is pressed, and says whether it did.
    pub(super) fn editor_reach_pressed(&mut self, modifiers: Modifiers) -> bool {
        if !modifiers.ctrl {
            return false;
        }
        let byte = self.editor_hovered();
        if let Some((_, target)) = self.link_at(byte) {
            self.model
                .change(|state| state.with_reach(None).link_followed(target));
            self.deliver();
            return true;
        }
        let Some(stretch) = self.word_at(byte) else {
            return false;
        };
        let at = self.hung(stretch.start, LISTING_HEIGHT);
        self.model.change(|state| {
            state
                .with_reach(None)
                .seek_requested(stretch.start, Seek::Definition, at)
        });
        self.deliver();
        true
    }

    /// The word of the file shown the pointer rests on, by where it starts, with how long it
    /// rests before the language server is asked about it: what the root's effect keyed on
    /// the word waits on.
    pub(super) fn hover_waiting(&self) -> Option<(usize, Duration)> {
        self.hover_rest.map(|word| (word, self.hover_rest_time()))
    }

    /// Asks the language server about the word starting at `word` once the pointer rested on
    /// it long enough, when it still rests there.
    pub(super) fn hover_rested(&mut self, word: usize) {
        if self.hover_rest != Some(word) {
            return;
        }
        self.hover_rest = None;
        self.hover_leaving = false;
        if let Some(at) = self.card_at(word) {
            self.model.change(|state| state.hover_requested(word, at));
        }
    }

    /// Whether the pointer left both the card about a word and the word while the card shows:
    /// what the root's effect waits on, taking the card away after [`HOVER_GRACE`].
    pub(super) fn hover_leaving(&self) -> bool {
        self.hover_leaving
    }

    /// Takes away the card about a word once the pointer stayed off it and its word for
    /// [`HOVER_GRACE`], when it is still away.
    pub(super) fn hover_left(&mut self) {
        if std::mem::take(&mut self.hover_leaving) {
            self.model.change(|state| state.hover_closed());
        }
    }
}

/// Whether the points `a` and `b` are the same, to within half a logical pixel.
fn same_point(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5
}

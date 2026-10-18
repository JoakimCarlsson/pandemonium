//! Writing code in the file shown: the keys that indent, complete and ask about a place, where the list of completions hangs, and what the language server is asked when the pointer rests on a word.

use std::time::{Duration, Instant};

use pm_acp::Verdict;
use pm_components::EditorCommand;
use pm_components::arrange::{
    SESSION_MENU_CLOSE, SESSION_MENU_COPY_BRANCH, SESSION_MENU_COPY_PATH, SESSION_MENU_OPEN_FOLDER,
    SESSION_MENU_RENAME,
};
use pm_components::assist::{
    HOVER_CARD, HOVER_LINK_BASE, HOVER_LINK_END, HOVER_TEXT, list_height, signature_height,
};
use pm_components::extensions::EXTENSIONS;
use pm_components::find::{FIND_CASE, FIND_FIELD, FIND_REGEX, FIND_WORD, REPLACE_FIELD};
use pm_components::line_edits::{GO_TO_LINE, GO_TO_LINE_BOX, LineEdit};
use pm_components::listing::{
    LISTING_BODY, LISTING_FILTER, LISTING_HEIGHT, LISTING_TAKE, LISTING_WIDTH, QUICK_INSET,
    listing_row_of,
};
use pm_components::navigate::{RENAME_BOX, RENAME_SYMBOL};
use pm_components::open_files::{EDITOR_FIELD, EDITOR_SAVE};
use pm_components::panes::{SPLIT_DOWN, SPLIT_LEFT, SPLIT_RIGHT, SPLIT_UP};
use pm_components::quick_choices::Choosing;
use pm_components::quick_input::{FIELD_HEIGHT, QUICK_SHOWN};
use pm_components::state::{
    ADD_PROJECT, HISTORY, NEW_TERMINAL, PROMPT, REVIEW, STOP, TAB_CHAT, TAB_DIFF, TERMINAL,
};
use pm_components::tab_menu::TabCommand;
use pm_components::title_bar::{COMMAND_CENTER, TITLE_BAR_HEIGHT};
use pm_config::ThemeMode;
use pm_editor::{Action, Chord, Keymap, Pressed};
use pm_input::{FocusId, Key, KeyEvent};
use pm_pages::Screen;
use pm_protocol::{DebugAction, Seek};
use pm_text::Edit;
use pm_theme::Appearance;

use super::App;
use crate::keys;

/// How long a card stays after the pointer left both it and its word, so the pointer can cross the gap between them.
const HOVER_GRACE: Duration = Duration::from_millis(350);

/// How many chords a binding recorded on the keymap page takes at most.
const RECORDED_CHORDS: usize = 2;

/// How long recording waits for a second chord before it keeps the first alone.
const RECORD_PAUSE: Duration = Duration::from_millis(1000);

/// How long a file waits after it was last changed before it saves itself, when the editor saves by itself.
const AUTOSAVE_REST: Duration = Duration::from_millis(1000);

/// How long the caret rests before the language server is asked where the thing at it is used.
const HIGHLIGHT_REST: Duration = Duration::from_millis(250);

/// How far below the line it is about the list or card hangs, in logical pixels.
const GAP: f32 = 2.0;

impl App {
    /// Where the byte `offset` of the file shown was drawn in the last frame: its left edge, and the top and height of its line.
    fn editor_point(&self, offset: usize) -> Option<(f32, f32, f32)> {
        let line = self
            .output
            .fields
            .iter()
            .filter(|held| held.id == EDITOR_FIELD && held.start <= offset)
            .max_by_key(|held| held.start)?;
        Some((
            line.x + line.run.caret_x(offset - line.start),
            line.y,
            line.height,
        ))
    }

    /// The byte of the file shown under the point `x`, `y`, when the point is on a drawn character.
    fn editor_byte_at(&self, x: f32, y: f32) -> Option<usize> {
        let line = self
            .output
            .fields
            .iter()
            .find(|held| held.id == EDITOR_FIELD && y >= held.y && y < held.y + held.height)?;
        if x < line.x || x > line.x + line.run.width {
            return None;
        }
        Some(line.start + line.run.byte_at(x - line.x, line.length))
    }

    /// How tall the window is, in logical pixels.
    fn window_height(&self) -> f32 {
        self.renderer
            .as_ref()
            .map_or(800.0, |renderer| renderer.size().1)
    }

    /// Hangs the list of completions under the word it completes, or above it when it would run past the bottom of the window, as the last frame drew the word.
    pub(super) fn place_completion(&mut self) {
        let Some((start, height)) = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .shown_document()
                .and_then(|document| document.completion.as_ref())
                .filter(|completion| !completion.loading)
                .map(|completion| (completion.start, list_height(completion))),
            Screen::Onboarding(_) => None,
        }) else {
            return;
        };
        let Some((x, top, line)) = self.editor_point(start) else {
            return;
        };
        let below = top + line + GAP;
        let at = if below + height > self.window_height() {
            (x, (top - height - GAP).max(0.0))
        } else {
            (x, below)
        };
        let placed = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .shown_document()
                .and_then(|document| document.completion.as_ref())
                .is_some_and(|completion| {
                    (completion.at.0 - at.0).abs() < 0.5 && (completion.at.1 - at.1).abs() < 0.5
                }),
            Screen::Onboarding(_) => true,
        });
        if !placed {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_completion_at(at));
        }
    }

    /// Hangs the signature of the call the caret is in just above the caret's line, or below it at the top of the window, as the last frame drew the line.
    pub(super) fn place_signature(&mut self) {
        let Some((caret, height)) = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state.shown_document().and_then(|document| {
                document
                    .signature
                    .as_ref()
                    .map(|signature| (document.field.caret(), signature_height(signature)))
            }),
            Screen::Onboarding(_) => None,
        }) else {
            return;
        };
        let Some((x, top, line)) = self.editor_point(caret) else {
            return;
        };
        let at = if top - height - GAP >= 0.0 {
            (x, top - height - GAP)
        } else {
            (x, top + line + GAP)
        };
        let placed = self.model.read(|screen| match screen {
            Screen::Workspace(state) => {
                (state.signature_at.0 - at.0).abs() < 0.5
                    && (state.signature_at.1 - at.1).abs() < 0.5
            }
            Screen::Onboarding(_) => true,
        });
        if !placed {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_signature_at(at));
        }
    }

    /// Saves the file shown once it has gone unchanged long enough with changes to save, when the editor saves by itself, and says when to look again while it has yet to.
    pub(super) fn editor_autosave_due(&mut self) -> Option<Instant> {
        let waiting = self.model.read(|screen| {
            let Screen::Workspace(state) = screen else {
                return None;
            };
            let document = state.shown_document()?;
            let dirty = state.editor.autosave && screen.shown_dirty();
            dirty.then(|| {
                (
                    state
                        .shown_viewer()
                        .map(|viewer| viewer.path.clone())
                        .unwrap_or_default(),
                    document.version,
                )
            })
        });
        let Some(here) = waiting else {
            self.autosave_wait = None;
            return None;
        };
        let since = match &self.autosave_wait {
            Some((held, since)) if *held == here => *since,
            _ => {
                let now = Instant::now();
                self.autosave_wait = Some((here, now));
                return Some(now + AUTOSAVE_REST);
            }
        };
        let due = since + AUTOSAVE_REST;
        if Instant::now() < due {
            return Some(due);
        }
        self.autosave_wait = None;
        self.activate(EDITOR_SAVE);
        None
    }

    /// Asks the language server where the thing at the caret of the file shown is used once the caret has rested there long enough, and says when to look again while it has yet to.
    pub(super) fn editor_highlight_due(&mut self) -> Option<Instant> {
        if self.focus.current() != Some(EDITOR_FIELD) {
            self.highlight_wait = None;
            return None;
        }
        let here = self.model.read(|screen| match screen {
            Screen::Workspace(state) => {
                let path = state.shown_viewer()?.path.clone();
                let document = state.shown_document()?;
                Some((path, document.version, document.field.caret()))
            }
            Screen::Onboarding(_) => None,
        })?;
        if self.highlighted.as_ref() == Some(&here) {
            self.highlight_wait = None;
            return None;
        }
        let since = match &self.highlight_wait {
            Some((waiting, since)) if *waiting == here => *since,
            _ => {
                let now = Instant::now();
                self.highlight_wait = Some((here, now));
                return Some(now + HIGHLIGHT_REST);
            }
        };
        let due = since + HIGHLIGHT_REST;
        if Instant::now() < due {
            return Some(due);
        }
        self.highlight_wait = None;
        self.highlighted = Some(here);
        self.model.update(|screen| {
            *screen = std::mem::take(screen)
                .highlights_requested()
                .annotations_requested()
                .folds_requested()
                .links_requested()
                .blame_requested()
        });
        None
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
        let at = self.below(caret);
        self.model
            .update(|screen| *screen = std::mem::take(screen).completion_requested(at));
    }

    /// Shows what is known about the place at the caret of the file shown.
    fn hover_at_caret(&mut self) {
        let Some(caret) = self
            .model
            .read(|screen| screen.field(EDITOR_FIELD).map(pm_text::Field::caret))
        else {
            return;
        };
        if let Some(at) = self.card_at(caret) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).hover_requested(caret, at));
        }
    }

    /// Copies the text selected in the card about a place when the keys that copy are pressed, and says whether they were.
    pub(super) fn card_key(&mut self, event: KeyEvent) -> bool {
        let selected = self
            .text_selection
            .is_some_and(|held| held.id == HOVER_TEXT && !held.is_empty());
        if !selected || keys::clip(&event) != Some(keys::Clip::Copy) {
            return false;
        }
        self.copy_selection();
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
        self.model
            .read(|screen| screen.field(EDITOR_FIELD).map(pm_text::Field::caret))
    }

    /// Where something hung under the line of the byte `offset` of the file shown goes: at its left edge, just below its line, as the last frame drew them.
    fn below(&self, offset: usize) -> (f32, f32) {
        self.editor_point(offset)
            .map_or((48.0, 96.0), |(x, top, line)| (x, top + line + GAP))
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
                .update(|screen| *screen = std::mem::take(screen).seek_requested(caret, seek, at));
            return;
        }
        self.model.update(|screen| {
            let taken = std::mem::take(screen);
            *screen = match command {
                EditorCommand::Rename => taken.rename_requested(caret, self.hung(caret, 72.0)),
                EditorCommand::Format => taken.formatted_by_hand(),
                EditorCommand::FormatSelection => taken.selection_formatted(),
                EditorCommand::CodeActions => taken.code_actions_requested(at),
                EditorCommand::CopyTrimmed => taken.copied_trimmed(),
                EditorCommand::Reveal => taken.file_revealed(),
                EditorCommand::OpenTerminal => taken.terminal_beside_file(),
                EditorCommand::Permalink => taken.permalink_requested(),
                EditorCommand::FileHistory => taken.file_history_requested(at),
                _ => taken,
            };
        });
        if command == EditorCommand::Rename && self.model.read(Screen::renaming_open) {
            self.focus.focus(RENAME_SYMBOL);
        }
        if command == EditorCommand::OpenTerminal {
            self.focus_terminal = true;
        }
    }

    /// The chord a press of keys makes, as the keymap names it.
    pub(super) fn chord_of(event: &KeyEvent) -> Option<Chord> {
        let modifiers = event.modifiers;
        let name = match event.key {
            Key::Character(character) => character.to_string(),
            Key::Enter => "enter".to_owned(),
            Key::Escape => "escape".to_owned(),
            Key::Tab => "tab".to_owned(),
            Key::Backspace => "backspace".to_owned(),
            Key::Delete => "delete".to_owned(),
            Key::Home => "home".to_owned(),
            Key::End => "end".to_owned(),
            Key::PageUp => "pageup".to_owned(),
            Key::PageDown => "pagedown".to_owned(),
            Key::Space => "space".to_owned(),
            Key::ArrowUp => "up".to_owned(),
            Key::ArrowDown => "down".to_owned(),
            Key::ArrowLeft => "left".to_owned(),
            Key::ArrowRight => "right".to_owned(),
            Key::Function(number) => format!("f{number}"),
        };
        Some(Chord::new(
            &name,
            modifiers.control || modifiers.logo,
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
        let focused = self.focus.current();
        action.everywhere()
            || focused == Some(EDITOR_FIELD)
            || (matches!(
                action,
                Action::Find | Action::Replace | Action::FindNext | Action::FindPrevious
            ) && (focused == Some(FIND_FIELD) || focused == Some(REPLACE_FIELD)))
    }

    /// Takes a press of keys while keys are recorded for an action on the keymap page: Escape gives up, anything else becomes its keys. Says whether it took the press.
    pub(super) fn recording_key(&mut self, event: KeyEvent) -> bool {
        if !self.model.read(Screen::recording_keys) {
            self.recorded.clear();
            return false;
        }
        if event.key == Key::Escape {
            self.recorded.clear();
            self.record_deadline = None;
            self.model
                .update(|screen| *screen = std::mem::take(screen).keys_recorded(None));
            return true;
        }
        let Some(chord) = Self::chord_of(&event) else {
            return true;
        };
        self.recorded.push(chord.written());
        if self.recorded.len() >= RECORDED_CHORDS {
            self.finish_recording();
        } else {
            self.record_deadline = Some(Instant::now() + RECORD_PAUSE);
            let so_far = self.recorded.clone();
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_recorded(so_far));
        }
        true
    }

    /// Keeps the chords recorded so far as the action's keys.
    fn finish_recording(&mut self) {
        self.record_deadline = None;
        let binding = std::mem::take(&mut self.recorded).join(" ");
        self.model.update(|screen| {
            *screen = std::mem::take(screen).keys_recorded((!binding.is_empty()).then_some(binding))
        });
        self.persist();
    }

    /// Keeps what was recorded once no further chord came in time, and says when to look again while one still may.
    pub(super) fn recording_due(&mut self) -> Option<Instant> {
        let due = self.record_deadline?;
        if Instant::now() < due {
            return Some(due);
        }
        self.finish_recording();
        None
    }

    /// Takes a press of keys the keymap binds to an action, or that starts a binding of more chords, and says whether it took it; typing goes on as typing.
    pub(super) fn keymap_key(&mut self, event: KeyEvent) -> bool {
        let modifiers = event.modifiers;
        let typing = matches!(event.key, Key::Character(_) | Key::Space)
            && !(modifiers.control || modifiers.logo || modifiers.alt);
        if typing && self.pending.is_empty() {
            return false;
        }
        let Some(chord) = Self::chord_of(&event) else {
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
        let center = self
            .output
            .regions
            .iter()
            .find(|region| region.id == COMMAND_CENTER)
            .map(|region| region.rect);
        let (middle, field_top) = center.map_or(
            (width / 2.0, (TITLE_BAR_HEIGHT - FIELD_HEIGHT) / 2.0),
            |rect| (rect.x + rect.width / 2.0, rect.y),
        );
        let left = (middle - LISTING_WIDTH / 2.0).clamp(0.0, (width - LISTING_WIDTH).max(0.0));
        (left, (field_top - QUICK_INSET).max(0.0))
    }

    /// Carries out the command the list takes when `id`, one of its rows or the key that takes the row picked, is pressed, and says whether the list held commands.
    pub(super) fn take_command(&mut self, id: FocusId) -> bool {
        let row = listing_row_of(id);
        if row.is_none() && id != LISTING_TAKE {
            return false;
        }
        let Some(action) = self.model.read(|screen| screen.command_taken(row)) else {
            let Some((host, command)) = self
                .model
                .read(|screen| screen.extension_command_taken(row))
            else {
                return false;
            };
            self.model.update(|screen| {
                *screen = std::mem::take(screen)
                    .listing_closed()
                    .extension_command_run(host, command)
            });
            self.leave_listing();
            self.deliver();
            return true;
        };
        self.model.update(|screen| {
            *screen = std::mem::take(screen).listing_closed().command_used(action)
        });
        self.leave_listing();
        self.perform(action);
        true
    }

    /// Gives the keyboard back to the file shown once the list over it closed, or lets go of it when no file is shown.
    fn leave_listing(&mut self) {
        if self
            .model
            .read(|screen| screen.field(EDITOR_FIELD).is_some())
        {
            self.focus.focus(EDITOR_FIELD);
        } else {
            self.focus.clear();
        }
    }

    /// Gives the keyboard to the field narrowing the list over the file once it is drawn, when one is open.
    fn focus_listing(&mut self) {
        if self.model.read(Screen::listing_open) {
            self.focus_filter = true;
        }
    }

    /// Opens the quick input over the choices of `choosing`.
    fn choose(&mut self, choosing: Choosing) {
        let at = self.across_top();
        self.model
            .update(|screen| *screen = std::mem::take(screen).choices_requested(choosing, at));
        self.focus_listing();
    }

    /// Presses the row `row` of the menu of the session shown.
    fn session_command(&mut self, row: FocusId) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).session_command(row));
        self.deliver();
    }

    /// Carries out `command` on the tab shown, as its menu does.
    fn tab_command(&mut self, command: TabCommand) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).shown_tab_command(command));
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
            Action::NextProblem => Some(LineEdit::NextProblem),
            Action::PreviousProblem => Some(LineEdit::PreviousProblem),
            Action::AddNextOccurrence => Some(LineEdit::AddNextOccurrence),
            Action::SelectAllOccurrences => Some(LineEdit::SelectAllOccurrences),
            Action::AddCaretAbove => Some(LineEdit::AddCaretAbove),
            Action::AddCaretBelow => Some(LineEdit::AddCaretBelow),
            Action::SelectLine => Some(LineEdit::SelectLine),
            Action::JoinLines => Some(LineEdit::JoinLines),
            Action::MatchingBracket => Some(LineEdit::MatchingBracket),
            Action::NewlineBelow => Some(LineEdit::LineBelow),
            Action::NewlineAbove => Some(LineEdit::LineAbove),
            Action::Fold => Some(LineEdit::Fold),
            Action::Unfold => Some(LineEdit::Unfold),
            _ => None,
        };
        if let Some(what) = edit {
            self.model
                .update(|screen| *screen = std::mem::take(screen).lines_edited(what));
            self.show_editor_caret();
            return;
        }
        match action {
            Action::Hover => self.hover_at_caret(),
            Action::Complete => self.complete_at_caret(),
            Action::SignatureHelp => self
                .model
                .update(|screen| *screen = std::mem::take(screen).signature_requested()),
            Action::GoBack | Action::GoForward => self.model.update(|screen| {
                let taken = std::mem::take(screen);
                *screen = if action == Action::GoForward {
                    taken.jumped_forward()
                } else {
                    taken.jumped_back()
                };
            }),
            Action::Find | Action::Replace => {
                let replacing = action == Action::Replace;
                self.model
                    .update(|screen| *screen = std::mem::take(screen).find_opened(replacing));
                self.focus.focus(FIND_FIELD);
                self.show_editor_caret();
            }
            Action::FindNext | Action::FindPrevious => {
                let back = action == Action::FindPrevious;
                self.model
                    .update(|screen| *screen = std::mem::take(screen).find_stepped(back));
                self.show_editor_caret();
            }
            Action::GoToLine => {
                let at = self
                    .editor_caret()
                    .map_or((48.0, 96.0), |caret| self.hung(caret, 72.0));
                self.model
                    .update(|screen| *screen = std::mem::take(screen).go_to_line_requested(at));
                self.focus.focus(GO_TO_LINE);
            }
            Action::Outline => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).outline_requested(at));
                self.focus_listing();
            }
            Action::ProjectSymbols => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).symbols_requested(at));
                self.focus_listing();
            }
            Action::FileFinder => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).file_finder_requested(at));
                self.focus_listing();
            }
            Action::SearchProject => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).project_search_opened());
                self.focus
                    .focus(pm_components::project_search::PROJECT_SEARCH_FIELD);
            }
            Action::Problems => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).problems_requested(at));
                self.focus_listing();
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
                self.focus_listing();
                self.deliver();
            }
            Action::AttachDebugger => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).debug_attach_requested(at));
                self.focus_listing();
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
            Action::Save => self.activate(EDITOR_SAVE),
            Action::CommandPalette => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).commands_requested(at));
                self.focus_listing();
            }
            Action::OpenSettings => self.open_settings(),
            Action::GoToSession => {
                let at = self.across_top();
                self.model
                    .update(|screen| *screen = std::mem::take(screen).sessions_requested(at));
                self.focus_listing();
            }
            Action::NewSession => {
                if let Some(id) = self.model.read(Screen::new_session_button) {
                    self.activate(id);
                }
            }
            Action::AddProject => self.activate(ADD_PROJECT),
            Action::ShowHistory => self.activate(HISTORY),
            Action::ShowExtensions => self.activate(EXTENSIONS),
            Action::WorkspaceBack => self.navigate(-1),
            Action::WorkspaceForward => self.navigate(1),
            Action::Quit => self.closing = self.close_window(),
            Action::ShowChat => self.activate(TAB_CHAT),
            Action::ShowDiff => self.activate(TAB_DIFF),
            Action::StopAgent => self.activate(STOP),
            Action::ToggleTerminal => self.activate(TERMINAL),
            Action::NewTerminal => self.activate(NEW_TERMINAL),
            Action::ToggleSourceControl => self.activate(REVIEW),
            Action::ChangeModel | Action::ChangeMode => {
                let category = if action == Action::ChangeModel {
                    "model"
                } else {
                    "mode"
                };
                let at = self.across_top();
                self.model.update(|screen| {
                    *screen = std::mem::take(screen).agent_category_requested(category, at)
                });
                self.focus_listing();
            }
            Action::ChangeAgentSetting => self.choose(Choosing::AgentSetting),
            Action::ChangeTheme => self.choose(Choosing::Theme),
            Action::CheckoutBranch | Action::CreateBranch => self.choose(Choosing::Branch),
            Action::ChangeKeymap => self.choose(Choosing::Keymap),
            Action::RewindConversation => self.choose(Choosing::Rewind),
            Action::ForkConversation => self.choose(Choosing::Fork),
            Action::RestoreFiles => self.choose(Choosing::Restore),
            Action::CycleMode => self
                .model
                .update(|screen| *screen = std::mem::take(screen).mode_cycled()),
            Action::AllowRequest | Action::AlwaysAllowRequest | Action::RejectRequest => {
                let verdict = match action {
                    Action::AllowRequest => Verdict::Allow,
                    Action::AlwaysAllowRequest => Verdict::Always,
                    _ => Verdict::Reject,
                };
                if let Some(id) = self.model.read(|screen| screen.answer_of(verdict)) {
                    self.activate(id);
                }
            }
            Action::RenameSession => self.session_command(SESSION_MENU_RENAME),
            Action::ArchiveSession => self.session_command(SESSION_MENU_CLOSE),
            Action::CopySessionPath => self.session_command(SESSION_MENU_COPY_PATH),
            Action::CopyBranchName => self.session_command(SESSION_MENU_COPY_BRANCH),
            Action::OpenSessionFolder => self.session_command(SESSION_MENU_OPEN_FOLDER),
            Action::FocusPrompt => self.focus.focus(PROMPT),
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
            Action::SplitRight => self.activate(SPLIT_RIGHT),
            Action::SplitLeft => self.activate(SPLIT_LEFT),
            Action::SplitUp => self.activate(SPLIT_UP),
            Action::SplitDown => self.activate(SPLIT_DOWN),
            Action::FocusNextPane | Action::FocusPreviousPane => {
                let delta = if action == Action::FocusNextPane {
                    1
                } else {
                    -1
                };
                self.model
                    .update(|screen| *screen = std::mem::take(screen).pane_stepped(delta));
            }
            Action::ToggleTheme => {
                let mode = if self.appearance == Appearance::Dark {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
                self.model
                    .update(|screen| *screen = std::mem::take(screen).with_theme_mode(mode));
            }
            Action::ZoomIn | Action::ZoomOut | Action::ResetZoom => {
                let steps = match action {
                    Action::ZoomIn => Some(1),
                    Action::ZoomOut => Some(-1),
                    _ => None,
                };
                self.model
                    .update(|screen| *screen = std::mem::take(screen).zoomed(steps));
            }
            Action::CopyPermalink => self.editor_command(EditorCommand::Permalink),
            Action::FileHistory => self.editor_command(EditorCommand::FileHistory),
            Action::OpenTerminalHere => self.editor_command(EditorCommand::OpenTerminal),
            Action::OutgoingCalls => self.editor_command(EditorCommand::OutgoingCalls),
            Action::ShowSupertypes => self.editor_command(EditorCommand::Supertypes),
            Action::ShowSubtypes => self.editor_command(EditorCommand::Subtypes),
            Action::CopyWithoutIndentation => self.editor_command(EditorCommand::CopyTrimmed),
            Action::Undo => self.edit_field(EDITOR_FIELD, Edit::Undo),
            Action::Redo => self.edit_field(EDITOR_FIELD, Edit::Redo),
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
                self.model
                    .update(|screen| *screen = std::mem::take(screen).source_control(action));
                self.deliver();
            }
            _ => {}
        }
    }

    /// Takes a key pressed while the box for a new name or for a line to go to has the keyboard: Escape closes it and gives the keyboard back to the file. Says whether it took the key.
    pub(super) fn rename_key(&mut self, event: KeyEvent) -> bool {
        let focused = self.focus.current();
        if event.key != Key::Escape
            || !(focused == Some(RENAME_SYMBOL) || focused == Some(GO_TO_LINE))
        {
            return false;
        }
        self.model
            .update(|screen| *screen = std::mem::take(screen).rename_closed().go_to_line_closed());
        self.focus.focus(EDITOR_FIELD);
        true
    }

    /// Takes a key pressed while the field narrowing the list over the file has the keyboard: the arrows move the pick and Escape closes the list. Says whether it took the key.
    pub(super) fn listing_key(&mut self, event: KeyEvent) -> bool {
        let focused = self.focus.current();
        let held = focused == Some(LISTING_FILTER) || focused.is_none();
        if !held || !self.model.read(Screen::listing_open) {
            return false;
        }
        let page = QUICK_SHOWN as isize - 1;
        let delta = match event.key {
            Key::ArrowUp => -1,
            Key::ArrowDown => 1,
            Key::PageUp => -page,
            Key::PageDown => page,
            Key::Escape => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).listing_closed());
                self.focus_filter = false;
                self.leave_listing();
                return true;
            }
            _ => return false,
        };
        self.model
            .update(|screen| *screen = std::mem::take(screen).listing_moved(delta));
        true
    }

    /// Takes a key pressed while the bar for finding has the keyboard: Escape closes it, Shift+Enter goes back a match, Ctrl+Alt+Enter replaces every match, and Alt with C, W or R switches matching case, whole words or regular expressions. Says whether it took the key.
    pub(super) fn find_key(&mut self, event: KeyEvent) -> bool {
        let focused = self.focus.current();
        if focused != Some(FIND_FIELD) && focused != Some(REPLACE_FIELD) {
            return false;
        }
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        let toggled = match event.key {
            Key::Character('c' | 'C') if modifiers.alt => Some(FIND_CASE),
            Key::Character('w' | 'W') if modifiers.alt => Some(FIND_WORD),
            Key::Character('r' | 'R') if modifiers.alt => Some(FIND_REGEX),
            _ => None,
        };
        if let Some(which) = toggled {
            self.model
                .update(|screen| *screen = std::mem::take(screen).find_toggled(which));
            self.show_editor_caret();
            return true;
        }
        match event.key {
            Key::Escape => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).find_closed());
                self.focus.focus(EDITOR_FIELD);
            }
            Key::Enter if focused == Some(REPLACE_FIELD) && command && modifiers.alt => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).replaced_all());
            }
            Key::Enter if focused == Some(FIND_FIELD) && modifiers.shift => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).find_stepped(true));
            }
            _ => return false,
        }
        self.show_editor_caret();
        true
    }

    /// What the keys of `event` that work as in any editor do to the lines or the selection of the file shown, when they do something, with whether the selection runs over lines, `spanning`: Home, Tab and Shift+Tab, and Backspace.
    fn line_edit_of(event: &KeyEvent, spanning: bool) -> Option<LineEdit> {
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        let (alt, shift) = (modifiers.alt, modifiers.shift);
        match event.key {
            Key::Home if !command && !alt => Some(LineEdit::Home(shift)),
            Key::Tab if shift && !command => Some(LineEdit::Outdent),
            Key::Tab if spanning && !command => Some(LineEdit::Indent),
            Key::Backspace if !command && !alt && !shift => Some(LineEdit::Backspace),
            _ => None,
        }
    }

    /// Closes the list over the file and the box for a new name when the press at `on` is outside them.
    pub(super) fn press_beside_code(&mut self, on: Option<pm_input::FocusId>) {
        let on_listing = on.is_some_and(|id| {
            id == LISTING_BODY || id == LISTING_FILTER || listing_row_of(id).is_some()
        });
        if !on_listing && self.model.read(Screen::listing_open) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).listing_closed());
        }
        let on_box = on.is_some_and(|id| id == RENAME_SYMBOL || id == RENAME_BOX);
        if !on_box && self.model.read(Screen::renaming_open) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).rename_closed());
        }
        let on_line_box = on.is_some_and(|id| id == GO_TO_LINE || id == GO_TO_LINE_BOX);
        if !on_line_box && self.model.read(Screen::going_to_open) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).go_to_line_closed());
        }
    }

    /// Gives the keyboard back to the file shown once the box for a new name or a line to go to it was lent to has closed.
    pub(super) fn settle_rename_focus(&mut self) {
        let focused = self.focus.current();
        let closed = (focused == Some(RENAME_SYMBOL) && !self.model.read(Screen::renaming_open))
            || (focused == Some(GO_TO_LINE) && !self.model.read(Screen::going_to_open))
            || ((focused == Some(FIND_FIELD) || focused == Some(REPLACE_FIELD))
                && !self.model.read(Screen::find_open))
            || (focused == Some(LISTING_FILTER) && !self.model.read(Screen::listing_open));
        if closed {
            self.focus.focus(EDITOR_FIELD);
            self.show_editor_caret();
        }
    }

    /// Takes a key pressed while the file shown has the keyboard, when it is one of the keys for writing code, and says whether it was.
    pub(super) fn editor_key(&mut self, event: KeyEvent) -> bool {
        if self.focus.current() != Some(EDITOR_FIELD) {
            return false;
        }
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        let spanning = self.model.read(Screen::selection_spans_lines);
        if event.key == Key::Tab
            && modifiers.shift
            && !command
            && self.model.read(Screen::snippet_active)
        {
            self.model
                .update(|screen| *screen = std::mem::take(screen).snippet_back());
            self.show_editor_caret();
            return true;
        }
        if let Some(what) = Self::line_edit_of(&event, spanning) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).lines_edited(what));
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
                .field(EDITOR_FIELD)
                .is_some_and(pm_text::Field::has_others)
        });
        if listing {
            let changed = |screen: Screen| -> Option<Screen> {
                match event.key {
                    Key::ArrowUp => Some(screen.listing_moved(-1)),
                    Key::ArrowDown => Some(screen.listing_moved(1)),
                    Key::Enter => Some(screen.listing_taken()),
                    Key::Escape => Some(screen.listing_closed()),
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
            self.model
                .update(|screen| *screen = std::mem::take(screen).listing_closed());
        }
        let next = |screen: Screen| -> Option<Screen> {
            match event.key {
                Key::ArrowUp if completing => Some(screen.completion_moved(-1)),
                Key::ArrowDown if completing => Some(screen.completion_moved(1)),
                Key::Enter | Key::Tab if completing && !modifiers.shift => {
                    Some(screen.completion_taken())
                }
                Key::Escape if completing => Some(screen.completion_closed()),
                Key::Escape if signing => Some(screen.signature_closed()),
                Key::Escape if snippeting => Some(screen.snippet_ended()),
                Key::Escape if finding => Some(screen.find_closed()),
                Key::Escape if several => Some(screen.lines_edited(LineEdit::OnlyCaret)),
                Key::Escape if hovering => Some(screen.hover_closed()),
                Key::Enter if !command && !modifiers.shift && !modifiers.alt => {
                    Some(screen.editor_newline())
                }
                Key::Tab if !modifiers.shift && !command => Some(screen.editor_tab()),
                _ => match keys::edit(&event) {
                    Some(Edit::Insert(character)) if !command => {
                        Some(screen.editor_typed(character))
                    }
                    _ => None,
                },
            }
        };
        if hovering && event.key != Key::Escape {
            self.model
                .update(|screen| *screen = std::mem::take(screen).hover_closed());
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

    /// Notes where the pointer rests over the file shown, so the language server is asked about the word there once it has rested long enough. A card about a word stays while the pointer is on the word or on the card; it is taken away a moment after the pointer left the word, so the pointer can cross to the card, and at once when the pointer leaves the card itself.
    pub(super) fn editor_pointer_moved(&mut self, x: f32, y: f32) {
        let over = self.output.hit((x, y));
        let on_card = over.is_some_and(|id| {
            id == HOVER_CARD || (HOVER_LINK_BASE..HOVER_LINK_END).contains(&id.0)
        });
        let selecting = self.text_held.is_some()
            && self
                .text_selection
                .is_some_and(|held| held.id == HOVER_TEXT);
        if on_card || selecting {
            self.hover_rest = None;
            self.hover_leaving = None;
            self.hover_entered |= on_card;
            return;
        }
        let stretch = self.word_under(over, x, y);
        self.reach(stretch.clone().filter(|_| self.modifiers.control));
        let word = stretch.map(|stretch| stretch.start);
        let shown = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .shown_document()
                .and_then(|document| document.hover.as_ref())
                .map(|card| card.offset),
            Screen::Onboarding(_) => None,
        });
        let left = shown.is_some_and(|offset| word != Some(offset));
        if left && std::mem::take(&mut self.hover_entered) {
            self.hover_leaving = None;
            self.model
                .update(|screen| *screen = std::mem::take(screen).hover_closed());
        } else {
            self.hover_leaving = left.then(|| self.hover_leaving.unwrap_or_else(Instant::now));
        }
        if shown.is_none() {
            self.hover_entered = false;
        }
        let word = word.filter(|_| self.hover_wanted());
        self.hover_rest = match (word, self.hover_rest) {
            (Some(start), _) if Some(start) == shown => None,
            (Some(start), Some((held, since))) if held == start => Some((held, since)),
            (Some(start), _) => Some((start, Instant::now())),
            (None, _) => None,
        };
    }

    /// The word of the file shown under the point `x`, `y`, where `over` is what the point is over, when it is on one.
    fn word_under(
        &self,
        over: Option<pm_input::FocusId>,
        x: f32,
        y: f32,
    ) -> Option<std::ops::Range<usize>> {
        let byte = (over == Some(EDITOR_FIELD))
            .then(|| self.editor_byte_at(x, y))
            .flatten()?;
        self.model.read(|screen| {
            let text = screen.field(EDITOR_FIELD)?.text();
            let word = pm_text::word_at(text, byte);
            text[word.clone()]
                .chars()
                .any(|character| character.is_alphanumeric() || character == '_')
                .then_some(word)
        })
    }

    /// Underlines `stretch` of the file shown as what a press takes to its definition, or nothing, when that differs from what is underlined.
    fn reach(&mut self, stretch: Option<std::ops::Range<usize>>) {
        let held = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .shown_document()
                .and_then(|document| document.reach.clone()),
            Screen::Onboarding(_) => None,
        });
        if held != stretch {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_reach(stretch));
        }
    }

    /// Whether a word of the file shown is underlined as what a press takes to its definition.
    pub(super) fn reaching(&self) -> bool {
        self.model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .shown_document()
                .is_some_and(|document| document.reach.is_some()),
            Screen::Onboarding(_) => false,
        })
    }

    /// Underlines the word under the pointer while Ctrl is held, and takes the underline away when it is let go.
    pub(super) fn editor_modifiers_changed(&mut self) {
        let stretch = self
            .pointer
            .position
            .filter(|_| self.modifiers.control)
            .and_then(|(x, y)| self.word_under(self.output.hit((x, y)), x, y));
        self.reach(stretch);
    }

    /// Takes the reader to where the word under the pointer is defined, when Ctrl is held as the file shown is pressed, and says whether it did.
    pub(super) fn editor_reach_pressed(&mut self) -> bool {
        if !self.modifiers.control {
            return false;
        }
        let Some((x, y)) = self.pointer.position else {
            return false;
        };
        let Some(stretch) = self.word_under(self.output.hit((x, y)), x, y) else {
            return false;
        };
        let at = self.hung(stretch.start, LISTING_HEIGHT);
        self.model.update(|screen| {
            *screen = std::mem::take(screen).with_reach(None).seek_requested(
                stretch.start,
                Seek::Definition,
                at,
            )
        });
        self.deliver();
        true
    }

    /// Takes away a card the pointer left long enough ago, asks about the word the pointer has rested on long enough, and says when to look again while either has yet to happen.
    pub(super) fn editor_hover_due(&mut self) -> Option<Instant> {
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        if let Some(left) = self.hover_leaving {
            let due = left + HOVER_GRACE;
            if now >= due {
                self.hover_leaving = None;
                self.model
                    .update(|screen| *screen = std::mem::take(screen).hover_closed());
            } else {
                next = Some(due);
            }
        }
        if let Some((start, since)) = self.hover_rest {
            let due = since + self.hover_rest_time();
            if now >= due {
                self.hover_rest = None;
                self.hover_leaving = None;
                if let Some(at) = self.card_at(start) {
                    self.model.update(|screen| {
                        *screen = std::mem::take(screen).hover_requested(start, at)
                    });
                }
            } else {
                next = Some(next.map_or(due, |held| held.min(due)));
            }
        }
        next
    }
}

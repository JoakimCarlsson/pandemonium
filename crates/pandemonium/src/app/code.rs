//! Writing code in the file shown: the keys that indent, complete and ask about a place, where the list of completions hangs, and what the language server is asked when the pointer rests on a word.

use std::time::{Duration, Instant};

use pm_components::EditorCommand;
use pm_components::assist::{
    HOVER_CARD, HOVER_LINK_BASE, HOVER_LINK_END, HOVER_TEXT, list_height, signature_height,
};
use pm_components::find::{FIND_CASE, FIND_FIELD, FIND_REGEX, FIND_WORD, REPLACE_FIELD};
use pm_components::line_edits::{GO_TO_LINE, GO_TO_LINE_BOX, LineEdit};
use pm_components::listing::{LISTING_BODY, LISTING_FILTER, LISTING_HEIGHT, listing_row_of};
use pm_components::navigate::{RENAME_BOX, RENAME_SYMBOL};
use pm_components::open_files::{EDITOR_FIELD, EDITOR_SAVE};
use pm_input::{Key, KeyEvent};
use pm_pages::Screen;
use pm_protocol::Seek;
use pm_text::Edit;

use super::App;
use crate::keys;

/// How long a card stays after the pointer left both it and its word, so the pointer can cross the gap between them.
const HOVER_GRACE: Duration = Duration::from_millis(350);

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

    /// The command the keys of `event` give for the file shown, when they give one; the second key of a pair begun with Ctrl+K when `paired`.
    fn command_of(event: &KeyEvent, paired: bool) -> Option<EditorCommand> {
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        if paired {
            return match event.key {
                Key::Character('r' | 'R') => Some(EditorCommand::Reveal),
                _ => None,
            };
        }
        match event.key {
            Key::Function(12) if modifiers.alt && modifiers.shift => {
                Some(EditorCommand::References)
            }
            Key::Function(12) if command => Some(EditorCommand::TypeDefinition),
            Key::Function(12) if modifiers.shift => Some(EditorCommand::Implementation),
            Key::Function(12) => Some(EditorCommand::Definition),
            Key::Function(2) => Some(EditorCommand::Rename),
            Key::Character('h' | 'H') if modifiers.alt && modifiers.shift => {
                Some(EditorCommand::IncomingCalls)
            }
            Key::Character('f' | 'F') if modifiers.alt && modifiers.shift => {
                Some(EditorCommand::Format)
            }
            Key::Character('i' | 'I') if command && modifiers.shift => Some(EditorCommand::Format),
            Key::Character('.') if command => Some(EditorCommand::CodeActions),
            _ => None,
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

    /// Opens the finder for the files of the project the review panel is about, across the top of the window, when Ctrl+P is pressed; says whether it was.
    pub(super) fn finder_key(&mut self, event: KeyEvent) -> bool {
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        if !command || modifiers.shift || !matches!(event.key, Key::Character('p' | 'P')) {
            return false;
        }
        let width = self
            .renderer
            .as_ref()
            .map_or(1280.0, |renderer| renderer.size().0);
        let at = (((width - 600.0) / 2.0).max(0.0), 56.0);
        self.model
            .update(|screen| *screen = std::mem::take(screen).file_finder_requested(at));
        if self.model.read(Screen::listing_open) {
            self.focus.focus(LISTING_FILTER);
        }
        true
    }

    /// Opens the search of the files of the project the review panel is about when Ctrl+Shift+F is pressed, or the list of what is wrong with them when Ctrl+Shift+M is, across the top of the window; says whether either was.
    pub(super) fn search_key(&mut self, event: KeyEvent) -> bool {
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        let searching = matches!(event.key, Key::Character('f' | 'F'));
        let problems = matches!(event.key, Key::Character('m' | 'M'));
        if !command || !modifiers.shift || !(searching || problems) {
            return false;
        }
        let width = self
            .renderer
            .as_ref()
            .map_or(1280.0, |renderer| renderer.size().0);
        let at = (((width - 600.0) / 2.0).max(0.0), 56.0);
        self.model.update(|screen| {
            let taken = std::mem::take(screen);
            *screen = if problems {
                taken.problems_requested(at)
            } else {
                taken.search_requested(at)
            };
        });
        if self.model.read(Screen::listing_open) {
            self.focus.focus(LISTING_FILTER);
        }
        true
    }

    /// Takes a key pressed while the field narrowing the list over the file has the keyboard: the arrows move the pick and Escape closes the list. Says whether it took the key.
    pub(super) fn listing_key(&mut self, event: KeyEvent) -> bool {
        if self.focus.current() != Some(LISTING_FILTER) {
            return false;
        }
        match event.key {
            Key::ArrowUp => self
                .model
                .update(|screen| *screen = std::mem::take(screen).listing_moved(-1)),
            Key::ArrowDown => self
                .model
                .update(|screen| *screen = std::mem::take(screen).listing_moved(1)),
            Key::Escape => {
                self.model
                    .update(|screen| *screen = std::mem::take(screen).listing_closed());
                self.focus.focus(EDITOR_FIELD);
            }
            _ => return false,
        }
        true
    }

    /// Takes a key pressed while the bar for finding has the keyboard: Escape closes it, Shift+Enter goes back a match, F3 steps, Ctrl+Alt+Enter replaces every match, and Alt with C, W or R switches matching case, whole words or regular expressions. Says whether it took the key.
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
            Key::Function(3) => {
                self.model.update(|screen| {
                    *screen = std::mem::take(screen).find_stepped(modifiers.shift)
                });
            }
            Key::Character('f' | 'F') if command => {
                self.edit_field(FIND_FIELD, Edit::SelectAll);
                self.focus.focus(FIND_FIELD);
            }
            _ => return false,
        }
        self.show_editor_caret();
        true
    }

    /// What the keys of `event` do to the lines or the selection of the file shown, when they do something, with whether the selection runs over lines, `spanning`.
    fn line_edit_of(event: &KeyEvent, spanning: bool) -> Option<LineEdit> {
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        let (alt, shift) = (modifiers.alt, modifiers.shift);
        match event.key {
            Key::Character('d' | 'D') if command && !shift => Some(LineEdit::AddNextOccurrence),
            Key::Character('l' | 'L') if command && shift => Some(LineEdit::SelectAllOccurrences),
            Key::ArrowUp if command && alt => Some(LineEdit::AddCaretAbove),
            Key::ArrowDown if command && alt => Some(LineEdit::AddCaretBelow),
            Key::Character('[' | '{') if command && shift => Some(LineEdit::Fold),
            Key::Character(']' | '}') if command && shift => Some(LineEdit::Unfold),
            Key::Character('/') if command => Some(LineEdit::ToggleComment),
            Key::Character('l' | 'L') if command && !shift => Some(LineEdit::SelectLine),
            Key::Character('j' | 'J') if command && !shift => Some(LineEdit::JoinLines),
            Key::Character('\\' | '|') if command && shift => Some(LineEdit::MatchingBracket),
            Key::Character('m' | 'M') if command && !shift => Some(LineEdit::MatchingBracket),
            Key::Enter if command && shift => Some(LineEdit::LineAbove),
            Key::Enter if command => Some(LineEdit::LineBelow),
            Key::Home if !command && !alt => Some(LineEdit::Home(shift)),
            Key::Character(']') if command => Some(LineEdit::Indent),
            Key::Character('[') if command => Some(LineEdit::Outdent),
            Key::Tab if shift && !command => Some(LineEdit::Outdent),
            Key::Tab if spanning && !command => Some(LineEdit::Indent),
            Key::ArrowUp if alt && shift => Some(LineEdit::DuplicateUp),
            Key::ArrowDown if alt && shift => Some(LineEdit::DuplicateDown),
            Key::ArrowUp if alt => Some(LineEdit::MoveUp),
            Key::ArrowDown if alt => Some(LineEdit::MoveDown),
            Key::ArrowRight if alt && shift => Some(LineEdit::Grow),
            Key::ArrowLeft if alt && shift => Some(LineEdit::Shrink),
            Key::Character('k' | 'K') if command && shift => Some(LineEdit::Delete),
            Key::Function(8) if shift => Some(LineEdit::PreviousProblem),
            Key::Function(8) => Some(LineEdit::NextProblem),
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
        let paired = std::mem::take(&mut self.paired);
        if paired {
            if matches!(event.key, Key::Character('i' | 'I')) {
                self.hover_at_caret();
                return true;
            }
            if let Some(chosen) = Self::command_of(&event, true) {
                self.editor_command(chosen);
                return true;
            }
        }
        let spanning = self.model.read(Screen::selection_spans_lines);
        if let Some(what) = Self::line_edit_of(&event, spanning) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).lines_edited(what));
            self.show_editor_caret();
            return true;
        }
        if command && matches!(event.key, Key::Character('f' | 'F' | 'h' | 'H')) && !modifiers.shift
        {
            let replacing = matches!(event.key, Key::Character('h' | 'H'));
            self.model
                .update(|screen| *screen = std::mem::take(screen).find_opened(replacing));
            self.focus.focus(FIND_FIELD);
            self.show_editor_caret();
            return true;
        }
        if event.key == Key::Function(3) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).find_stepped(modifiers.shift));
            self.show_editor_caret();
            return true;
        }
        if command && !modifiers.shift && matches!(event.key, Key::Character('t' | 'T')) {
            let width = self
                .renderer
                .as_ref()
                .map_or(1280.0, |renderer| renderer.size().0);
            let at = (((width - 600.0) / 2.0).max(0.0), 56.0);
            self.model
                .update(|screen| *screen = std::mem::take(screen).symbols_requested(at));
            if self.model.read(Screen::listing_open) {
                self.focus.focus(LISTING_FILTER);
            }
            return true;
        }
        if command && modifiers.shift && matches!(event.key, Key::Character('o' | 'O')) {
            let at = self.editor_caret().map_or((48.0, 96.0), |caret| {
                self.hung(caret, LISTING_HEIGHT + 40.0)
            });
            self.model
                .update(|screen| *screen = std::mem::take(screen).outline_requested(at));
            self.focus.focus(LISTING_FILTER);
            return true;
        }
        if command && matches!(event.key, Key::Character('g' | 'G')) {
            let at = self
                .editor_caret()
                .map_or((48.0, 96.0), |caret| self.hung(caret, 72.0));
            self.model
                .update(|screen| *screen = std::mem::take(screen).go_to_line_requested(at));
            self.focus.focus(GO_TO_LINE);
            return true;
        }
        if command && !modifiers.shift && matches!(event.key, Key::Character('k' | 'K')) {
            self.paired = true;
            return true;
        }
        let (completing, hovering, listing, signing, finding) = self.model.read(|screen| {
            (
                screen.completion_open(),
                screen.hover_open(),
                screen.listing_open(),
                screen.signature_open(),
                screen.find_open(),
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
        if let Some(chosen) = Self::command_of(&event, false) {
            self.editor_command(chosen);
            return true;
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
                Key::Escape if finding => Some(screen.find_closed()),
                Key::Escape if several => Some(screen.lines_edited(LineEdit::OnlyCaret)),
                Key::Escape if hovering => Some(screen.hover_closed()),
                Key::Enter if !command && !modifiers.shift && !modifiers.alt => {
                    Some(screen.editor_newline())
                }
                Key::Tab if !modifiers.shift && !command => Some(screen.editor_indent()),
                _ => match keys::edit(&event) {
                    Some(Edit::Insert(character)) if !command => {
                        Some(screen.editor_typed(character))
                    }
                    _ => None,
                },
            }
        };
        if command && modifiers.shift && event.key == Key::Space {
            self.model
                .update(|screen| *screen = std::mem::take(screen).signature_requested());
            return true;
        }
        if command && event.key == Key::Space {
            self.complete_at_caret();
            return true;
        }
        if command && matches!(event.key, Key::Character('-' | '_')) {
            self.model.update(|screen| {
                let taken = std::mem::take(screen);
                *screen = if modifiers.shift {
                    taken.jumped_forward()
                } else {
                    taken.jumped_back()
                };
            });
            return true;
        }
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

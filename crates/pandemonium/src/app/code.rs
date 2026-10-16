//! Writing code in the file shown: the keys that indent, complete and ask about a place, where the list of completions hangs, and what the language server is asked when the pointer rests on a word.

use std::time::{Duration, Instant};

use pm_components::assist::{HOVER_CARD, list_height};
use pm_components::open_files::EDITOR_FIELD;
use pm_input::{Key, KeyEvent};
use pm_pages::Screen;
use pm_text::Edit;

use super::App;
use crate::keys;

/// How long the pointer rests on a word before the language server is asked about it.
const HOVER_REST: Duration = Duration::from_millis(450);

/// How far below the line it is about the list or card hangs, in logical pixels.
const GAP: f32 = 2.0;

/// How tall a card about a place is taken to be when deciding whether it fits under the line, in logical pixels.
const CARD_ROOM: f32 = 220.0;

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

    /// Where a card about the byte `offset` hangs: under its line, or above it near the bottom of the window.
    fn card_at(&self, offset: usize) -> Option<(f32, f32)> {
        let (x, top, line) = self.editor_point(offset)?;
        let below = top + line + GAP;
        Some(if below + CARD_ROOM > self.window_height() {
            (x, (top - CARD_ROOM).max(0.0))
        } else {
            (x, below)
        })
    }

    /// Asks for completions at the caret of the file shown.
    fn complete_at_caret(&mut self) {
        let Some(caret) = self
            .model
            .read(|screen| screen.field(EDITOR_FIELD).map(pm_text::Field::caret))
        else {
            return;
        };
        let at = self
            .editor_point(caret)
            .map_or((0.0, 0.0), |(x, top, line)| (x, top + line + GAP));
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

    /// Takes a key pressed while the file shown has the keyboard, when it is one of the keys for writing code, and says whether it was.
    pub(super) fn editor_key(&mut self, event: KeyEvent) -> bool {
        if self.focus.current() != Some(EDITOR_FIELD) {
            return false;
        }
        let modifiers = event.modifiers;
        let command = modifiers.control || modifiers.logo;
        let (completing, hovering) = self
            .model
            .read(|screen| (screen.completion_open(), screen.hover_open()));
        let next = |screen: Screen| -> Option<Screen> {
            match event.key {
                Key::ArrowUp if completing => Some(screen.completion_moved(-1)),
                Key::ArrowDown if completing => Some(screen.completion_moved(1)),
                Key::Enter | Key::Tab if completing && !modifiers.shift => {
                    Some(screen.completion_taken())
                }
                Key::Escape if completing => Some(screen.completion_closed()),
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
        if command && event.key == Key::Space {
            self.complete_at_caret();
            return true;
        }
        if command && matches!(event.key, Key::Character('k' | 'K')) {
            self.hover_at_caret();
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

    /// Notes where the pointer rests over the file shown, so the language server is asked about the word there once it has rested long enough, and takes away a card about a word it left.
    pub(super) fn editor_pointer_moved(&mut self, x: f32, y: f32) {
        let over = self.output.hit((x, y));
        if over == Some(HOVER_CARD) {
            self.hover_rest = None;
            return;
        }
        let word = (over == Some(EDITOR_FIELD))
            .then(|| self.editor_byte_at(x, y))
            .flatten()
            .and_then(|byte| {
                self.model.read(|screen| {
                    let text = screen.field(EDITOR_FIELD)?.text();
                    let word = pm_text::word_at(text, byte);
                    text[word.clone()]
                        .chars()
                        .any(|character| character.is_alphanumeric() || character == '_')
                        .then_some(word.start)
                })
            });
        if self.model.read(Screen::hover_open) && word.is_none() {
            self.model
                .update(|screen| *screen = std::mem::take(screen).hover_closed());
        }
        self.hover_rest = match (word, self.hover_rest) {
            (Some(start), Some((held, since))) if held == start => Some((held, since)),
            (Some(start), _) => Some((start, Instant::now())),
            (None, _) => None,
        };
    }

    /// Asks about the word the pointer has rested on long enough, and says when to look again while it has not yet.
    pub(super) fn editor_hover_due(&mut self) -> Option<Instant> {
        let (start, since) = self.hover_rest?;
        let due = since + HOVER_REST;
        if Instant::now() < due {
            return Some(due);
        }
        self.hover_rest = None;
        if let Some(at) = self.card_at(start) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).hover_requested(start, at));
        }
        None
    }
}

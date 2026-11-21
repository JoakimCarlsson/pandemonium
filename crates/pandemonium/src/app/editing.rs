//! What the window does for the file shown between its frames: asking after the changes the language server offers once the caret rests, carrying selected text to another place by the pointer, and moving the caret along the screen in right-to-left text.

use pm_components::areas::EditorEvent;
use pm_components::target::Target;
use std::time::Instant;

use pm_components::text_lines::Step;
use pm_editor::LightbulbShown;
use pm_editor::{Edit, Motion};
use rustle::{Key, KeyEvent};

use super::App;

/// The file, version, caret, other end of the selection and number of problems that the marker beside the caret's line was asked about.
pub(super) type BulbKey = (String, i32, usize, usize, usize);

/// How long the caret rests before the language server is asked whether it offers changes there.
const BULB_REST: std::time::Duration = std::time::Duration::from_millis(250);

/// Selected text of the file shown that the button went down on, which dragging carries to another place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TextDrag {
    /// Where the button went down, in logical pixels of the window.
    origin: (f32, f32),
    /// The byte of the text the button went down on.
    byte: usize,
    /// Whether the pointer has gone far enough from where it went down for this to be a drag and not a press.
    moving: bool,
    /// How far the pointer has to move, in logical pixels, for this to be a drag: the touch slop of the view configuration in effect where the text was pressed.
    touch_slop: f32,
}

impl App {
    /// Asks the language server whether it offers changes at the caret of the file shown once the caret has rested there long enough, and says when to look again while it has yet to.
    pub(super) fn editor_bulb_due(&mut self) -> Option<Instant> {
        let here = if self.focus.focused() == Some(Target::Editor(EditorEvent::EditorField)) {
            self.model.read(|state| {
                if state.editor.lightbulb == LightbulbShown::Off {
                    return None;
                }
                let path = state.shown_viewer()?.path.clone();
                let document = state.shown_document()?;
                Some((
                    path,
                    document.version,
                    document.field.caret(),
                    document.field.anchor(),
                    document.diagnostics.len(),
                ))
            })
        } else {
            None
        };
        let Some(here) = here else {
            self.bulb_wait = None;
            return None;
        };
        if self.bulb_asked.as_ref() == Some(&here) {
            self.bulb_wait = None;
            return None;
        }
        let since = match &self.bulb_wait {
            Some((waiting, since)) if *waiting == here => *since,
            _ => {
                let now = Instant::now();
                self.bulb_wait = Some((here, now));
                return Some(now + BULB_REST);
            }
        };
        let due = since + BULB_REST;
        if Instant::now() < due {
            return Some(due);
        }
        self.bulb_wait = None;
        self.bulb_asked = Some(here);
        self.model.change(|state| state.lightbulb_requested());
        None
    }

    /// Has the shaper join characters into ligatures, or not, as the editor's settings say, drawing everything again when that changes.
    pub(super) fn follow_ligatures(&mut self) {
        let Some(on) = self
            .model
            .read(|screen| screen.editor_settings().map(|settings| settings.ligatures))
        else {
            return;
        };
        if pm_components::theme::code_ligatures() != on {
            pm_components::theme::set_code_ligatures(on);
            self.recompose = true;
        }
    }

    /// Takes the press in the file shown at the byte `byte`, at the point `at` of the window, as the start of carrying the selected text when it is on the selection and dragging text is on, a drag once the pointer moved `touch_slop` logical pixels; says whether it did, in which case the press is not a selection of its own.
    pub(super) fn drag_text_begins(
        &mut self,
        byte: usize,
        at: (f32, f32),
        touch_slop: f32,
    ) -> bool {
        let mut picked = false;
        self.model.update(|state| {
            picked = state.editor_drag_started(byte);
        });
        self.text_drag = picked.then_some(TextDrag {
            origin: at,
            byte,
            moving: false,
            touch_slop,
        });
        picked
    }

    /// Carries the selected text being dragged to the point `(x, y)` of the window, over the shown offset `offset` of the file shown: once the pointer has gone far enough it marks where the text would be dropped.
    pub(super) fn drag_text_moved(&mut self, (x, y): (f32, f32), offset: usize) {
        let Some(mut drag) = self.text_drag else {
            return;
        };
        if !drag.moving && (x - drag.origin.0).hypot(y - drag.origin.1) < drag.touch_slop {
            return;
        }
        drag.moving = true;
        self.text_drag = Some(drag);
        self.model.update(|state| {
            let byte = state
                .shown_document()
                .map(|document| document.field.to_text(offset));
            if let Some(byte) = byte {
                state.editor_drag_over(byte);
            }
        });
    }

    /// Ends the dragging of selected text as the button comes up: the text is moved to where it was dropped, or copied there when Ctrl is held; a press that never moved puts the caret where it was pressed. Says whether there was text being carried.
    pub(super) fn drag_text_released(&mut self) -> bool {
        let Some(drag) = self.text_drag.take() else {
            return false;
        };
        if drag.moving {
            let copy = self.modifiers.ctrl || self.modifiers.meta;
            self.model.update(|state| {
                state.editor_dropped(copy);
            });
            self.show_editor_caret();
        } else {
            self.model.update(|state| {
                state.editor_drag_cancelled();
            });
            self.edit_field(
                Target::Editor(EditorEvent::EditorField),
                Edit::Select(drag.byte, drag.byte),
            );
        }
        true
    }

    /// Lets go of the selected text being dragged, leaving it where it was.
    pub(super) fn drag_text_cancelled(&mut self) {
        if self.text_drag.take().is_some() {
            self.model.update(|state| {
                state.editor_drag_cancelled();
            });
        }
    }

    /// Takes a press of the left or right arrow key in the file shown when the caret is in a line with right-to-left text and the editor moves it along the screen; says whether it did. A selection that is not being extended is left to the ordinary keys, which go to its start or its end.
    pub(super) fn move_along_screen(&mut self, event: &KeyEvent) -> bool {
        let right = match event.key {
            Key::ArrowRight => true,
            Key::ArrowLeft => false,
            _ => return false,
        };
        let modifiers = event.modifiers;
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            return false;
        }
        let extend = modifiers.shift;
        let on = self
            .model
            .read(|screen| screen.editor_settings().map(|s| s.visual_caret_movement));
        if on != Some(true) {
            return false;
        }
        let Some((caret, selected, several)) = self.model.read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .map(|field| {
                    (
                        field.to_shown(field.caret()),
                        field.selection().is_some(),
                        field.has_others(),
                    )
                })
        }) else {
            return false;
        };
        if several || (selected && !extend) {
            return false;
        }
        match self
            .code_lines()
            .visual_step(Target::Editor(EditorEvent::EditorField), caret, right)
        {
            None => false,
            Some(Step::To(target)) => {
                self.edit_field(
                    Target::Editor(EditorEvent::EditorField),
                    Edit::Place(target, extend),
                );
                true
            }
            Some(Step::Edge) => {
                if let Some(edge) = self.code_lines().line_edge(
                    Target::Editor(EditorEvent::EditorField),
                    caret,
                    right,
                ) {
                    self.edit_field(
                        Target::Editor(EditorEvent::EditorField),
                        Edit::Place(edge, extend),
                    );
                }
                let motion = if right { Motion::Right } else { Motion::Left };
                self.edit_field(
                    Target::Editor(EditorEvent::EditorField),
                    Edit::Move(motion, extend),
                );
                true
            }
        }
    }
}

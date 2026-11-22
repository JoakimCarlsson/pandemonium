//! What the window does for the file shown between its frames: asking after the changes the language server offers once the caret rests, carrying selected text to another place with rustle's drag and drop, and moving the caret along the screen in right-to-left text.

use pm_components::areas::EditorEvent;
use pm_components::target::Target;

use pm_components::text_lines::Step;
use pm_editor::LightbulbShown;
use pm_editor::{Edit, Motion};
use rustle::{Key, KeyEvent};

use super::App;

/// The file, version, caret, other end of the selection and number of problems that the marker beside the caret's line was asked about.
pub(super) type BulbKey = (String, i32, usize, usize, usize);

/// How long the caret rests before the language server is asked whether it offers changes there.
pub(super) const BULB_REST: std::time::Duration = std::time::Duration::from_millis(250);

impl App {
    /// The file, version, caret, other end of the selection and number of problems of the
    /// file shown while the code editor has the keyboard, the marker beside the caret's line
    /// is on and the language server was not yet asked about them: what the root's effect
    /// waits on, asking once the caret rested there long enough.
    pub(super) fn bulb_waiting(&self) -> Option<BulbKey> {
        if self.focus.focused() != Some(Target::Editor(EditorEvent::EditorField)) {
            return None;
        }
        let here = self.model.read(|state| {
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
        })?;
        (self.bulb_asked.as_ref() != Some(&here)).then_some(here)
    }

    /// Asks the language server whether it offers changes at the caret once the caret rested
    /// at `here` for [`BULB_REST`], when it still waits there.
    pub(super) fn bulb_rested(&mut self, here: BulbKey) {
        if self.bulb_waiting().as_ref() != Some(&here) {
            return;
        }
        self.bulb_asked = Some(here);
        self.model.change(|state| state.lightbulb_requested());
    }

    /// Has the shaper join characters into ligatures, or not, as the editor's settings say; the composables that set code read the switch as snapshot state and are composed again when it changes.
    pub(super) fn follow_ligatures(&mut self) {
        let Some(on) = self
            .model
            .read(|screen| screen.editor_settings().map(|settings| settings.ligatures))
        else {
            return;
        };
        pm_components::theme::set_code_ligatures(on);
    }

    /// Takes the press in the file shown at the byte `byte` as one on the selection that a
    /// drag carries to another place, when dragging text is on and the press is inside the
    /// selection; says whether it did, in which case the press is not a selection of its own.
    pub(super) fn text_pressed(&mut self, byte: usize) -> bool {
        let picks = self.model.read(|screen| match screen {
            pm_pages::Screen::Workspace(state) => state.editor_picks_up(byte),
        });
        self.text_press = picks.then_some(byte);
        picks
    }

    /// Picks up the selection pressed on as rustle's drag and drop starts a drag from the
    /// press, and says whether it did.
    pub(super) fn text_drag_started(&mut self) -> bool {
        let Some(byte) = self.text_press.take() else {
            return false;
        };
        let mut picked = false;
        self.model.update(|state| {
            picked = state.editor_drag_started(byte);
        });
        picked
    }

    /// Marks where the selected text being carried would be put down: before the shown
    /// offset `offset` of the file shown.
    pub(super) fn text_dragged_over(&mut self, offset: usize) {
        self.model.update(|state| {
            let byte = state
                .shown_document()
                .map(|document| document.field.to_text(offset));
            if let Some(byte) = byte {
                state.editor_drag_over(byte);
            }
        });
    }

    /// Puts the selected text being carried down where it was last over as the drag ends:
    /// moved there, or copied when Ctrl is held in `modifiers`.
    pub(super) fn text_drag_ended(&mut self, modifiers: rustle::Modifiers) {
        if !self.text_dragging() {
            return;
        }
        let copy = modifiers.ctrl || modifiers.meta;
        self.model.update(|state| {
            state.editor_dropped(copy);
        });
        self.show_editor_caret();
    }

    /// Puts the caret where the selection was pressed as the button comes up without the
    /// press having become a drag.
    pub(super) fn text_released(&mut self) {
        if let Some(byte) = self.text_press.take() {
            self.edit_field(
                Target::Editor(EditorEvent::EditorField),
                Edit::Select(byte, byte),
            );
        }
    }

    /// Whether selected text of the file shown is being carried.
    pub(super) fn text_dragging(&self) -> bool {
        self.model.read(|screen| match screen {
            pm_pages::Screen::Workspace(state) => state.editor_dragging(),
        })
    }

    /// Lets go of the selected text being carried, leaving it where it was.
    pub(super) fn text_drag_cancelled(&mut self) {
        self.model.update(|state| {
            state.editor_drag_cancelled();
        });
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

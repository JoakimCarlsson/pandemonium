//! What a keypress, a click or a wheel notch does to the window.
//!
//! A keypress is resolved against the keymap first and falls back to focus
//! movement; a pointer event goes to the element tree, which answers with the
//! screen's own message. Either way the window ends up with one message or a
//! redraw, never with a widget reaching into the state behind its back.

use pm_gfx::Point;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

use crate::app::App;
use crate::keymap::{self, Action, Context, Resolution, keys};
use crate::onboarding::Message;

/// Logical pixels one notch of a mouse wheel scrolls.
pub(super) const WHEEL_STEP: f32 = 48.0;

/// How many notches a page key scrolls.
const PAGE_NOTCHES: f32 = 4.0;

impl App {
    /// What is true where a key was pressed, for the `when` clauses to read.
    fn context(&self) -> Context {
        let mut context = Context::new();
        context.flag(keys::SETUP_OPEN, !self.setup.finished);
        context
    }

    /// Carries `action` out, ignoring the ones nothing is built behind yet.
    fn act(&mut self, action: Action) {
        match action {
            Action::OpenSettings => self.apply(Message::Reopen),
            Action::Cancel => {
                if let Some(ui) = self.ui.as_mut() {
                    ui.clear_focus();
                }
                self.request_redraw();
            }
            _ => self.request_redraw(),
        }
    }

    /// Resolves a keypress against the keymap, falling back to focus movement.
    pub(super) fn key_pressed(&mut self, event: &KeyEvent) {
        if let Some(chord) = keymap::chord(event, self.modifiers) {
            match self.resolver.press(chord, &self.context()) {
                Resolution::Act(action) => return self.act(action),
                Resolution::Pending => return self.request_redraw(),
                Resolution::None => {}
            }
        }
        self.navigate(event);
    }

    /// Moves focus, activates what has it, or scrolls the page.
    fn navigate(&mut self, event: &KeyEvent) {
        let Some(ui) = self.ui.as_mut() else {
            return;
        };

        let message = match event.logical_key {
            Key::Named(NamedKey::Tab) if self.modifiers.shift_key() => {
                ui.focus_previous();
                None
            }
            Key::Named(NamedKey::Tab) => {
                ui.focus_next();
                None
            }
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => ui.activate_focused(),
            Key::Named(NamedKey::PageDown) => {
                self.scroll_by(-WHEEL_STEP * PAGE_NOTCHES);
                None
            }
            Key::Named(NamedKey::PageUp) => {
                self.scroll_by(WHEEL_STEP * PAGE_NOTCHES);
                None
            }
            _ => None,
        };

        self.handle(message);
    }

    /// Tells the element tree where the pointer is now.
    pub(super) fn pointer_moved(&mut self, position: Point) {
        if let Some(ui) = self.ui.as_mut() {
            ui.pointer_moved(position);
        }
        self.request_redraw();
    }

    /// Tells the element tree the pointer has left the window.
    pub(super) fn pointer_left(&mut self) {
        if let Some(ui) = self.ui.as_mut() {
            ui.pointer_left();
        }
        self.request_redraw();
    }

    /// Presses or releases the primary button, applying what it activated.
    pub(super) fn pointer_button(&mut self, state: ElementState) {
        let message = match (self.ui.as_mut(), state) {
            (Some(ui), ElementState::Pressed) => {
                ui.pointer_pressed();
                None
            }
            (Some(ui), ElementState::Released) => ui.pointer_released(),
            (None, _) => None,
        };
        self.handle(message);
    }

    /// Scrolls the page by `delta` logical pixels and redraws.
    pub(super) fn scroll_by(&mut self, delta: f32) {
        self.scroll.by(delta);
        self.request_redraw();
    }

    /// Applies a message the input produced, or redraws when it produced none.
    fn handle(&mut self, message: Option<Message>) {
        match message {
            Some(message) => self.apply(message),
            None => self.request_redraw(),
        }
    }
}

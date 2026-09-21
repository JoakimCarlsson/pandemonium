//! What a keypress, a click or a wheel notch does to the window.
//!
//! A keypress is resolved against the keymap first and falls back to focus
//! movement; a pointer event goes to the element tree, which answers with the
//! screen's own message. Either way the window ends up with one message or a
//! redraw, never with a widget reaching into the state behind its back.

use pm_gfx::Point;
#[cfg(not(target_os = "macos"))]
use pm_gfx::Size;
use pm_ui::PointerCursor;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};
#[cfg(not(target_os = "macos"))]
use winit::window::ResizeDirection;

use crate::app::App;
use crate::editor;
use crate::keymap::{self, Action, Context, Resolution, keys};
use crate::onboarding::Message;
use crate::terminal;

/// Logical pixels one notch of a mouse wheel scrolls.
pub(super) const WHEEL_STEP: f32 = 48.0;

/// Logical pixels one row of a terminal occupies, for turning a wheel notch
/// into a number of lines to scroll back.
const TERMINAL_ROW: f32 = 19.6;

/// Logical pixels one line of the editor occupies, for the same reason.
const EDITOR_ROW: f32 = 21.0;

/// How many notches a page key scrolls.
const PAGE_NOTCHES: f32 = 4.0;

/// Longest interval treated as a double click.
pub(super) const DOUBLE_CLICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Width of the invisible resize target around an undecorated window.
#[cfg(not(target_os = "macos"))]
const WINDOW_RESIZE_EDGE: f32 = 5.0;

impl App {
    /// What is true where a key was pressed, for the `when` clauses to read.
    fn context(&self) -> Context {
        let mut context = Context::new();
        context.flag(keys::SETUP_OPEN, !self.setup.finished);
        context.flag(keys::PROJECT_FOCUSED, self.open.active().is_some());
        if let Some(kind) = self.focused_pane_kind() {
            context.set(keys::PANE_KIND, kind);
        }
        context
    }

    /// Carries `action` out, ignoring the ones nothing is built behind yet.
    fn act(&mut self, action: Action) {
        match action {
            Action::OpenSettings => self.apply(Message::Reopen),
            Action::AddProject => self.apply(Message::OpenProject),
            Action::RemoveProject => {
                if let Some(id) = self.open.active().map(pm_core::Project::id) {
                    self.apply(Message::CloseProject(id));
                }
            }
            Action::Save => {
                if let Some(project) = self.open.active().map(pm_core::Project::id) {
                    self.editor.save(project);
                }
                self.request_redraw();
            }
            Action::SaveAll => {
                self.editor.save_all();
                self.request_redraw();
            }
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
        if self.send_to_terminal(event) {
            return self.request_redraw();
        }
        if let Some(chord) = keymap::chord(event, self.modifiers) {
            match self.resolver.press(chord, &self.context()) {
                Resolution::Act(action) => return self.act(action),
                Resolution::Pending => return self.request_redraw(),
                Resolution::None => {}
            }
        }
        if self.send_to_editor(event) {
            return self.request_redraw();
        }
        self.navigate(event);
    }

    /// Sends a keypress to the editor pane, when the pane has the keyboard.
    ///
    /// The pane takes it after the keymap has had its say, so a chord the
    /// window binds stays the window's however deep in a file the cursor is.
    fn send_to_editor(&mut self, event: &KeyEvent) -> bool {
        if self.is_window_chord() {
            return false;
        }
        let Some(file) = self.focused_file() else {
            return false;
        };
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return false;
        };
        let rows = file.borrow().rows();
        let Some(edit) = editor::edit(&event.logical_key, self.modifiers, rows) else {
            return false;
        };

        self.editor.edit(project, |buffer| match edit {
            editor::Edit::Insert(text) => buffer.insert(&text),
            editor::Edit::Newline => buffer.insert_newline(),
            editor::Edit::Indent => buffer.insert_indent(),
            editor::Edit::Backspace => buffer.backspace(),
            editor::Edit::Delete => buffer.delete(),
            editor::Edit::Move(motion, extend) => buffer.move_cursor(motion, extend),
            editor::Edit::SelectAll => buffer.select_all(),
        });
        true
    }

    /// Sends a keypress to the terminal, when the terminal has the keyboard.
    ///
    /// A focused terminal takes almost every key: Escape, Tab and Ctrl-C
    /// belong to the program running in it rather than to the window. What it
    /// does not take are the window's own chords — the ones on the platform
    /// key or on Ctrl-Shift — so the panel can still be closed from the
    /// keyboard while a program is running in it.
    fn send_to_terminal(&mut self, event: &KeyEvent) -> bool {
        if self.is_window_chord() {
            return false;
        }
        let Some(shell) = self.focused_shell() else {
            return false;
        };
        let Some(key) = terminal::key(&event.logical_key) else {
            return false;
        };
        shell
            .borrow_mut()
            .press(key, terminal::modifiers(self.modifiers))
    }

    /// Whether the modifiers held mark this keypress as the window's own.
    fn is_window_chord(&self) -> bool {
        self.modifiers.super_key() || (self.modifiers.control_key() && self.modifiers.shift_key())
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
        self.pointer = Some(position);
        let message = self.ui.as_mut().and_then(|ui| ui.pointer_moved(position));
        self.update_pointer_cursor();
        self.handle(message);
    }

    /// Tells the element tree the pointer has left the window.
    pub(super) fn pointer_left(&mut self) {
        self.pointer = None;
        if let Some(ui) = self.ui.as_mut() {
            ui.pointer_left();
        }
        self.update_pointer_cursor();
        self.request_redraw();
    }

    /// Presses or releases the primary button, applying what it activated.
    pub(super) fn pointer_button(&mut self, state: ElementState) {
        #[cfg(not(target_os = "macos"))]
        if state == ElementState::Pressed
            && let (Some(pointer), Some(renderer), Some(window)) =
                (self.pointer, self.renderer.as_ref(), self.window.as_ref())
            && let Some(direction) = window_resize_direction(pointer, renderer.size())
        {
            let _ = window.drag_resize_window(direction);
            return;
        }

        if state == ElementState::Pressed
            && self.setup.finished
            && self
                .pointer
                .is_some_and(|pointer| pointer.y < crate::workspace::TITLEBAR_HEIGHT)
            && !self.ui.as_ref().is_some_and(|ui| ui.pointer_over_region())
        {
            if let Some(window) = self.window.as_ref() {
                let now = std::time::Instant::now();
                if self
                    .last_titlebar_click
                    .is_some_and(|last| now.duration_since(last) <= DOUBLE_CLICK_INTERVAL)
                {
                    self.last_titlebar_click = None;
                    window.set_maximized(!window.is_maximized());
                    return;
                }
                self.last_titlebar_click = Some(now);
                let _ = window.drag_window();
            }
            return;
        }

        if state == ElementState::Pressed {
            self.release_pane_focus();
        }

        let message = match (self.ui.as_mut(), state) {
            (Some(ui), ElementState::Pressed) => ui.pointer_pressed(),
            (Some(ui), ElementState::Released) => ui.pointer_released(),
            (None, _) => None,
        };
        self.update_pointer_cursor();
        self.handle(message);
    }

    /// Scrolls the page by `delta` logical pixels and redraws.
    ///
    /// A focused terminal scrolls its own scrollback instead: the page behind
    /// it does not move while the pointer is working in the pane.
    pub(super) fn scroll_by(&mut self, delta: f32) {
        if let Some(shell) = self.focused_shell() {
            let lines = (delta / TERMINAL_ROW).round() as isize;
            shell.borrow_mut().scroll(lines);
            self.request_redraw();
            return;
        }
        if let Some(file) = self.focused_file() {
            let lines = (delta / EDITOR_ROW).round() as isize;
            file.borrow_mut().scroll_by(-lines);
            self.request_redraw();
            return;
        }
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

    /// Applies the cursor requested by the current hover or drag target.
    fn update_pointer_cursor(&self) {
        let cursor = self
            .ui
            .as_ref()
            .map_or(PointerCursor::Default, |ui| ui.pointer_cursor());
        let icon = match cursor {
            PointerCursor::Default => winit::window::CursorIcon::Default,
            PointerCursor::Pointer => winit::window::CursorIcon::Pointer,
            PointerCursor::ResizeHorizontal => winit::window::CursorIcon::ColResize,
            PointerCursor::ResizeVertical => winit::window::CursorIcon::RowResize,
            PointerCursor::Text => winit::window::CursorIcon::Text,
        };
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(icon);
        }
    }
}

/// Returns the native resize direction for a pointer along a window edge.
#[cfg(not(target_os = "macos"))]
fn window_resize_direction(pointer: Point, size: Size) -> Option<ResizeDirection> {
    let left = pointer.x <= WINDOW_RESIZE_EDGE;
    let right = pointer.x >= size.width - WINDOW_RESIZE_EDGE;
    let top = pointer.y <= WINDOW_RESIZE_EDGE;
    let bottom = pointer.y >= size.height - WINDOW_RESIZE_EDGE;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

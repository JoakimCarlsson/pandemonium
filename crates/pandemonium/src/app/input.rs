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

use crate::app::{App, Writing};
use crate::editor::{self, Completions};
use crate::field::Typed;
use crate::keymap::{self, Action, Context, Resolution, keys};
use crate::message::Message;
use crate::terminal;

/// Logical pixels one notch of a mouse wheel scrolls.
pub(super) const WHEEL_STEP: f32 = 48.0;

/// How many notches a page key scrolls.
const PAGE_NOTCHES: f32 = 4.0;

/// How many rows of a picker a page key moves through.
const PICKER_PAGE: isize = 10;

/// How long the pointer stays still before the editor asks about what is
/// under it.
///
/// The question goes out well before the panel is due, so that the answer is
/// usually already in hand by the time the reader has stopped long enough to
/// want one.
const REST_DELAY: std::time::Duration = std::time::Duration::from_millis(150);

/// Longest interval treated as a double click.
pub(super) const DOUBLE_CLICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Width of the invisible resize target around an undecorated window.
///
/// An undecorated window has no frame outside itself to grab, so the whole
/// target is inside the window and has to be wide enough to find without
/// aiming: a decorated window gets about this much, plus its own border.
#[cfg(not(target_os = "macos"))]
const WINDOW_RESIZE_EDGE: f32 = 8.0;

impl App {
    /// What is true where a key was pressed, for the `when` clauses to read.
    pub(super) fn context(&self) -> Context {
        let mut context = Context::new();
        context.flag(keys::SETUP_OPEN, !self.onboarded);
        context.flag(keys::PROJECT_FOCUSED, self.open.active().is_some());
        if let Some(kind) = self.focused_pane_kind() {
            context.set(keys::PANE_KIND, kind);
        }
        context
    }

    /// Resolves a keypress against whatever has the keyboard.
    ///
    /// The order is what is nearest the reader first: a list open over the
    /// screen, then the completions offered beside the cursor, then a
    /// terminal, then the window's own chords, then the search bar, then the
    /// text itself. Only a key nothing wanted becomes focus movement.
    pub(super) fn key_pressed(&mut self, event: &KeyEvent) {
        self.blink.restart();
        if self.send_to_prompt(event) {
            return self.request_redraw();
        }
        if self.send_to_picker(event) {
            return self.request_redraw();
        }
        if self.send_to_completions(event) {
            return self.request_redraw();
        }
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
        if self.send_to_input(event) {
            return self.request_redraw();
        }
        if self.send_to_changes(event) {
            return self.request_redraw();
        }
        if self.send_to_search(event) {
            return self.request_redraw();
        }
        if self.send_to_editor(event) {
            return self.request_redraw();
        }
        self.navigate(event);
    }

    /// Sends a keypress to the question the window is asking, if it is asking.
    ///
    /// The question is modal, so it answers before anything else does and
    /// takes every key: a chord that would otherwise act on the file behind
    /// it is a chord aimed at a window that is waiting for an answer.
    fn send_to_prompt(&mut self, event: &KeyEvent) -> bool {
        if self.prompt.is_none() {
            return false;
        }
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => {
                self.dismiss_prompt();
            }
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => self.answer_prompt(),
            Key::Named(NamedKey::ArrowUp) => self.step_prompt(-1),
            Key::Named(NamedKey::ArrowDown) => self.step_prompt(1),
            Key::Named(NamedKey::Tab) if self.modifiers.shift_key() => self.step_prompt(-1),
            Key::Named(NamedKey::Tab) => self.step_prompt(1),
            _ => {}
        }
        true
    }

    /// Moves the question's answer `steps` along.
    fn step_prompt(&mut self, steps: isize) {
        if let Some(asked) = self.prompt.as_mut() {
            asked.step(steps);
        }
    }

    /// Answers the question the way it is showing.
    fn answer_prompt(&mut self) {
        let taken = self.prompt.take().and_then(|asked| asked.chosen());
        if let Some(taken) = taken {
            self.apply(taken);
        }
    }

    /// Sends a keypress to the list the window is asking a choice from.
    fn send_to_picker(&mut self, event: &KeyEvent) -> bool {
        if self.picker.is_none() || self.is_window_chord() {
            return false;
        }
        let modifiers = self.modifiers;
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => self.dismiss_picker(),
            Key::Named(NamedKey::Enter) => {
                self.confirm_picker();
                true
            }
            Key::Named(NamedKey::ArrowUp) => self.step_picker(-1),
            Key::Named(NamedKey::ArrowDown) => self.step_picker(1),
            Key::Named(NamedKey::PageUp) => self.step_picker(-(PICKER_PAGE)),
            Key::Named(NamedKey::PageDown) => self.step_picker(PICKER_PAGE),
            key => {
                let Some(picker) = self.picker.as_mut() else {
                    return false;
                };
                let mut taken = false;
                picker.edit(|field| taken = field.press(&key, modifiers) == Typed::Taken);
                if taken {
                    self.refilter_picker();
                }
                taken
            }
        }
    }

    /// Moves the picker's selection, saying that the key was taken.
    fn step_picker(&mut self, step: isize) -> bool {
        if let Some(picker) = self.picker.as_mut() {
            picker.step(step);
        }
        true
    }

    /// Sends a keypress to the completions offered beside the cursor.
    ///
    /// Only the keys that work the list are taken: everything else goes on
    /// into the buffer and narrows the list afterwards, which is what makes
    /// completion happen beside the typing rather than instead of it.
    fn send_to_completions(&mut self, event: &KeyEvent) -> bool {
        if self.completions.is_none() {
            return false;
        }
        let step = match event.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => {
                self.completions = None;
                return true;
            }
            Key::Named(NamedKey::ArrowUp) => -1,
            Key::Named(NamedKey::ArrowDown) => 1,
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab) => {
                let place = self.completions.as_ref().map_or(0, Completions::selected);
                self.take_completion(place);
                return true;
            }
            _ => return false,
        };
        if let Some(completions) = self.completions.as_mut() {
            completions.step(step);
        }
        true
    }

    /// Sends a keypress to the box of text that has the keyboard.
    ///
    /// Every box takes the same keys, so there is one of these however many
    /// boxes the window has: what differs is the key that finishes a box and
    /// what finishing it means, and both of those belong to the box.
    ///
    /// The one thing that comes before the box is the list of commands a
    /// slash has narrowed to, because while it is up the arrows and Enter are
    /// choosing from it rather than writing.
    fn send_to_input(&mut self, event: &KeyEvent) -> bool {
        let Some(writing) = self.writing else {
            return false;
        };
        if let Writing::Prompt(session) = writing
            && self.choosing_command(session, event)
        {
            return true;
        }

        let (key, modifiers) = (event.logical_key.clone(), self.modifiers);
        if self
            .written_in()
            .is_some_and(|input| input.submits(&key, modifiers))
        {
            self.submit_writing(writing);
            return true;
        }
        if self.is_window_chord() {
            return false;
        }

        let Some(input) = self.written_in() else {
            return false;
        };
        if input.press(&key, modifiers) == Typed::Ignored {
            return false;
        }
        if let Writing::Prompt(session) = writing
            && let Some(talk) = self.agents.get_mut(session)
        {
            talk.retyped();
        }
        true
    }

    /// Takes a key the list of commands a slash narrowed to wanted.
    fn choosing_command(&mut self, session: crate::agent::TalkId, event: &KeyEvent) -> bool {
        if !self.naming_command(session) {
            return false;
        }
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::ArrowUp) => self.step_command(session, -1),
            Key::Named(NamedKey::ArrowDown) => self.step_command(session, 1),
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.take_chosen();
                }
                true
            }
            _ => false,
        }
    }

    /// Whether `session`'s prompt is naming one of the agent's commands.
    fn naming_command(&self, session: crate::agent::TalkId) -> bool {
        self.agents
            .get(session)
            .is_some_and(|talk| !talk.offered().is_empty())
    }

    /// Moves `session`'s selection `by` rows through the commands offered.
    fn step_command(&mut self, session: crate::agent::TalkId, by: isize) -> bool {
        if let Some(talk) = self.agents.get_mut(session) {
            talk.step_command(by);
        }
        true
    }

    /// Does what finishing the box that has the keyboard means.
    fn submit_writing(&mut self, writing: Writing) {
        match writing {
            Writing::Commit => self.apply(Message::Commit),
            Writing::Prompt(session) => self.apply(Message::SendPrompt(session)),
        }
    }

    /// Sends a keypress to the list of changes, when the list has the keyboard.
    ///
    /// The list works the way every list of files does: the arrows move the
    /// row it is on and shift with them marks everything they pass, Enter
    /// opens what is under it, Space stages it or takes it back out, and
    /// Escape lets go of the marks before it lets go of the list.
    fn send_to_changes(&mut self, event: &KeyEvent) -> bool {
        if !self.changes_focused || self.is_window_chord() {
            return false;
        }
        let marking = self.modifiers.shift_key();

        match event.logical_key.as_ref() {
            Key::Named(NamedKey::ArrowUp) => self.step_changes(-1, marking),
            Key::Named(NamedKey::ArrowDown) => self.step_changes(1, marking),
            Key::Named(NamedKey::Enter) => self.open_selected_change(),
            Key::Named(NamedKey::Space) => self.toggle_selection_staged(),
            Key::Named(NamedKey::Escape) => {
                if !self.clear_change_marks() {
                    self.changes_focused = false;
                }
            }
            _ => return false,
        }
        true
    }

    /// Sends a keypress to the search bar, when the bar has the keyboard.
    fn send_to_search(&mut self, event: &KeyEvent) -> bool {
        if !self.search_focused || self.is_window_chord() {
            return false;
        }
        let Some(file) = self.active_file() else {
            return false;
        };
        if !file.borrow().search().is_open() {
            self.search_focused = false;
            return false;
        }
        let modifiers = self.modifiers;

        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Enter) if modifiers.shift_key() => {
                self.act(Action::FindPrevious);
            }
            Key::Named(NamedKey::Enter) => {
                let replacing =
                    file.borrow().search().field() == crate::editor::SearchField::Replacement;
                self.act(if replacing {
                    Action::ReplaceMatch
                } else {
                    Action::FindNext
                });
            }
            Key::Named(NamedKey::Tab) => {
                file.borrow_mut().search_with(|search, _| {
                    let next = match search.field() {
                        crate::editor::SearchField::Query => {
                            crate::editor::SearchField::Replacement
                        }
                        crate::editor::SearchField::Replacement => {
                            crate::editor::SearchField::Query
                        }
                    };
                    search.focus(next);
                });
            }
            key => {
                let mut taken = false;
                file.borrow_mut().search_with(|search, buffer| {
                    search.edit_field(
                        |field| taken = field.press(&key, modifiers) == Typed::Taken,
                        buffer,
                    );
                });
                if !taken {
                    return false;
                }
            }
        }
        true
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
        let rows = file.borrow().rows();
        let Some(edit) = editor::edit(&event.logical_key, self.modifiers, rows) else {
            return false;
        };

        let typed = match edit {
            editor::Edit::Type(ch) => Some(ch),
            _ => None,
        };
        let line_wise = matches!(edit, editor::Edit::Indent | editor::Edit::Outdent);
        self.edit_active(|buffer| {
            let apply = |buffer: &mut pm_text::Buffer| match edit.clone() {
                editor::Edit::Type(ch) => buffer.insert_typed(ch),
                editor::Edit::Insert(text) => buffer.insert(&text),
                editor::Edit::Newline => buffer.insert_newline(),
                editor::Edit::Indent => buffer.insert_indent(),
                editor::Edit::Outdent => buffer.outdent_lines(),
                editor::Edit::Backspace => buffer.backspace(),
                editor::Edit::Delete => buffer.delete(),
                editor::Edit::DeleteWordLeft => buffer.delete_word_left(),
                editor::Edit::DeleteWordRight => buffer.delete_word_right(),
                editor::Edit::Move(motion, extend) => buffer.move_cursor(motion, extend),
            };
            if line_wise {
                buffer.on_each_line(apply);
            } else {
                buffer.at_each(apply);
            }
        });
        self.after_typing(typed);
        true
    }

    /// Sends a keypress to the terminal, when the terminal has the keyboard.
    ///
    /// A focused terminal takes almost every key: Escape, Tab and Ctrl-C
    /// belong to the program running in it rather than to the window. What it
    /// does not take are the window's own chords — the ones on the platform
    /// key or on Ctrl-Shift — so the panel can still be closed from the
    /// keyboard while a program is running in it. The clipboard's keys come
    /// before either, because copy and paste mean the terminal's own text.
    fn send_to_terminal(&mut self, event: &KeyEvent) -> bool {
        let Some(shell) = self.focused_shell() else {
            return false;
        };
        let selected = shell.borrow().selection_span().is_some();
        if let Some(action) = terminal::clipboard(&event.logical_key, self.modifiers, selected) {
            return self.act_on_terminal(action);
        }
        if self.is_window_chord() {
            return false;
        }
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

    /// Opens the menu of whatever the secondary button was pressed on.
    ///
    /// A press on nothing that answers to the button is how a menu is
    /// dismissed as well: the sheet under an open menu answers, so the only
    /// presses that reach here with nothing to open are presses with no menu
    /// over them.
    pub(super) fn secondary_pressed(&mut self) {
        match self.ui.as_ref().and_then(|ui| ui.secondary_pressed()) {
            Some(message) => self.apply(message),
            None => {
                self.dismiss_menu();
                self.request_redraw();
            }
        }
    }

    /// Goes back or forward along the trail from the mouse's own thumb
    /// buttons.
    ///
    /// The pane the pointer is over takes the jump, and the focus with it, the
    /// way it does in Zed: a thumb button is aimed at what is under it, while
    /// the keyboard's back and forward belong to whatever has the focus. A
    /// list open over the screen takes the window's attention first, and the
    /// buttons do nothing while it is up, as the keys they stand in for do.
    pub(super) fn travelled(&mut self, back: bool) {
        if !self.onboarded || self.picker.is_some() {
            return;
        }
        let action = if back {
            Action::GoBack
        } else {
            Action::GoForward
        };
        match self.pointer.and_then(|point| self.geometry.pane_at(point)) {
            Some(pane) => self.apply(Message::PaneAction(pane, action)),
            None => self.act(action),
        }
    }

    /// Tells the element tree where the pointer is now.
    ///
    /// Every move starts the clock again, at the place the pointer has
    /// reached: a clock left running from where the pointer set off would
    /// go off about a place it has long since left, which is how a reader
    /// creeping onto a name gets an answer about the blank they started
    /// from. What was being said about the word the pointer has left goes
    /// away; what is said about the word it is still on stays, and so does
    /// the link it is still over.
    pub(super) fn pointer_moved(&mut self, position: Point) {
        let moved = self.pointer != Some(position);
        self.pointer = Some(position);
        if moved {
            self.forget_hint(position);
            self.resting = Some((std::time::Instant::now(), position));
        }
        self.follow_pointer(position);
        let message = self.ui.as_mut().and_then(|ui| ui.pointer_moved(position));
        self.update_pointer_cursor();
        self.handle(message);
    }

    /// Says what the editor knows about a place the pointer has rested on.
    ///
    /// The delay is what tells resting from passing over: a pointer crossing
    /// a line of code on its way somewhere is not asking about every word it
    /// crossed.
    pub(super) fn rested(&mut self) -> bool {
        let Some((since, at)) = self.resting else {
            return false;
        };
        if since.elapsed() < REST_DELAY {
            return false;
        }
        self.resting = None;
        self.hover_at(at);
        true
    }

    /// How long the window should wait before looking at the clock again.
    pub(super) fn next_rest(&self) -> Option<std::time::Instant> {
        self.resting.map(|(since, _)| since + REST_DELAY)
    }

    /// Whether the caret has turned over since the last frame.
    ///
    /// Only a pane with the keyboard has a caret to blink; a window whose
    /// text is not being edited is a window that stays still.
    pub(super) fn blinked(&mut self) -> bool {
        self.editor_focused && self.blink.changed()
    }

    /// When the caret next turns over, while there is one to turn.
    pub(super) fn next_blink(&self) -> Option<std::time::Instant> {
        self.editor_focused.then(|| self.blink.next_change())
    }

    /// Tells the element tree the pointer has left the window.
    ///
    /// A tab being carried is put back when the pointer leaves: the release
    /// that would have dropped it happens somewhere this window will never
    /// hear about, and a tab stuck to a pointer that is not there is worse
    /// than one that stayed where it was.
    pub(super) fn pointer_left(&mut self) {
        self.pointer = None;
        self.drag = None;
        self.resting = None;
        self.hint = None;
        self.link = None;
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
            && !window.is_maximized()
            && let Some(direction) = window_resize_direction(pointer, renderer.size())
        {
            let _ = window.drag_resize_window(direction);
            return;
        }

        if state == ElementState::Pressed
            && self.onboarded
            && self
                .pointer
                .is_some_and(|pointer| pointer.y < self.theme().size.titlebar)
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
        if state == ElementState::Released {
            self.release_drag();
        }
    }

    /// Scrolls the page by `delta` logical pixels and redraws.
    ///
    /// A focused terminal scrolls its own scrollback instead: the page behind
    /// it does not move while the pointer is working in the pane.
    pub(super) fn scroll_by(&mut self, delta: f32) {
        let text = self.theme().text;
        if self.secondary_sidebar_open
            && self.secondary_sidebar_view == crate::workspace::SidebarView::Changes
            && self.history_graph_open
            && self
                .pointer
                .is_some_and(|pointer| self.history_graph_bounds.get().contains(pointer))
        {
            let row_height = self.theme().size.row;
            let rows = (delta / row_height).round() as isize;
            if rows != 0 {
                let visible = ((self.history_graph.extent() - row_height) / row_height)
                    .floor()
                    .max(1.0) as usize;
                let all = self.history_all;
                if let Some(review) = self.review_mut() {
                    review.scroll_history(all, -rows, visible);
                }
                self.request_redraw();
                return;
            }
        }
        if let Some(shell) = self.focused_shell() {
            let lines = (delta / text.terminal.line_height).round() as isize;
            shell.borrow_mut().scroll(lines);
            self.request_redraw();
            return;
        }
        if self.scroll_settings(delta) {
            self.request_redraw();
            return;
        }
        let rows = (delta / text.code.line_height).round() as isize;
        if self.scroll_agent(-rows) {
            self.request_redraw();
            return;
        }
        if self.scroll_review(-rows) {
            self.request_redraw();
            return;
        }
        let under = self
            .pointer
            .and_then(|pointer| self.document_at(pointer))
            .map(|(_, document)| document)
            .or_else(|| self.focused_file());
        if let Some(file) = under {
            let lines = (delta / text.code.line_height).round() as isize;
            file.borrow_mut().scroll_by(-lines);
            self.request_redraw();
            return;
        }
        self.scroll.by(delta);
        self.request_redraw();
    }

    /// Scrolls the focused pane along its lines by `delta` logical pixels.
    pub(super) fn scroll_across(&mut self, delta: f32) {
        let under = self
            .pointer
            .and_then(|pointer| self.document_at(pointer))
            .map(|(_, document)| document)
            .or_else(|| self.active_file());
        let Some(file) = under else {
            return;
        };
        let width = file.borrow().layout().cell.width.max(1.0);
        let columns = (delta / width).round() as isize;
        file.borrow_mut().scroll_columns(-columns);
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
    pub(super) fn update_pointer_cursor(&self) {
        let icon = self.window_resize_cursor().unwrap_or_else(|| {
            let cursor = self
                .ui
                .as_ref()
                .map_or(PointerCursor::Default, |ui| ui.pointer_cursor());
            match cursor {
                PointerCursor::Default => winit::window::CursorIcon::Default,
                PointerCursor::Pointer => winit::window::CursorIcon::Pointer,
                PointerCursor::ResizeHorizontal => winit::window::CursorIcon::ColResize,
                PointerCursor::ResizeVertical => winit::window::CursorIcon::RowResize,
                PointerCursor::Text => winit::window::CursorIcon::Text,
            }
        });
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(icon);
        }
    }

    /// The shape the pointer takes along an undecorated window's own edges.
    ///
    /// The edge is invisible, so the cursor is the only thing that says it is
    /// there: without it a window that resizes perfectly well reads as one
    /// that does not resize at all.
    #[cfg(not(target_os = "macos"))]
    fn window_resize_cursor(&self) -> Option<winit::window::CursorIcon> {
        if self
            .window
            .as_ref()
            .is_some_and(|window| window.is_maximized())
        {
            return None;
        }
        let pointer = self.pointer?;
        let size = self.renderer.as_ref()?.size();
        window_resize_direction(pointer, size).map(|direction| match direction {
            ResizeDirection::North | ResizeDirection::South => winit::window::CursorIcon::NsResize,
            ResizeDirection::East | ResizeDirection::West => winit::window::CursorIcon::EwResize,
            ResizeDirection::NorthWest | ResizeDirection::SouthEast => {
                winit::window::CursorIcon::NwseResize
            }
            ResizeDirection::NorthEast | ResizeDirection::SouthWest => {
                winit::window::CursorIcon::NeswResize
            }
        })
    }

    /// A decorated window resizes from its own frame, which macOS draws.
    #[cfg(target_os = "macos")]
    fn window_resize_cursor(&self) -> Option<winit::window::CursorIcon> {
        None
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

//! Input and pane wiring for a worktree's live file outline.

use pm_core::Scope;
use pm_ui::{Div, Theme};
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

use crate::app::App;
use crate::keymap::{self, Resolution};
use crate::message::Message;
use crate::outline;
use crate::panes::{Item, PaneId};

impl App {
    /// Builds the outline pane from the file last focused in `scope`.
    pub(super) fn outline_content(
        &self,
        theme: &Theme,
        pane: PaneId,
        scope: Scope,
    ) -> Div<Message> {
        let file = self.outlines.followed(scope);
        let state = file.and_then(|file| self.outlines.get(file));
        outline::outline_pane(theme, scope, state, self.panes.focus() == pane)
    }

    /// Handles outline clicks before the general message dispatcher.
    pub(super) fn apply_outline(&mut self, message: Message) -> bool {
        match message {
            Message::OpenOutline(pane, file) => {
                self.activate_tab(pane, Item::File(file));
                self.open_outline();
            }
            Message::OutlineSelect(scope, index) => self.jump_to_outline(scope, index),
            Message::OutlineToggle(scope, index) => {
                self.focus_outline(scope);
                if let Some(file) = self.outlines.followed(scope) {
                    self.outlines.get_mut(file).toggle(index);
                }
            }
            Message::WriteOutlineFilter(scope, phase, anchor, head) => {
                self.focus_outline(scope);
                self.point_focused_input(phase, anchor, head);
            }
            _ => return false,
        }
        true
    }

    /// Gives the keyboard to the pane showing the worktree's outline.
    fn focus_outline(&mut self, scope: Scope) {
        let pane = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.active(Some(scope)) == Some(Item::Outline(scope)))
        });
        if let Some(pane) = pane {
            self.focus_pane(pane);
        }
    }

    /// Sends unmodified keys to the focused outline before modal editing.
    pub(super) fn send_to_outline(&mut self, event: &KeyEvent) -> bool {
        let Some(Item::Outline(scope)) = self.active_tab() else {
            return false;
        };
        if self.modifiers.control_key() || self.modifiers.super_key() || self.modifiers.alt_key() {
            return false;
        }
        if event.logical_key != Key::Named(NamedKey::Escape)
            && let Some(chord) = keymap::chord(event, self.modifiers)
        {
            match self.resolver.press(chord, &self.context()) {
                Resolution::Act(action) => {
                    self.act(action);
                    return true;
                }
                Resolution::Pending => return true,
                Resolution::None => {}
            }
        }
        let Some(file) = self.outlines.followed(scope) else {
            if event.logical_key == Key::Named(NamedKey::Escape) {
                return false;
            }
            return true;
        };
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => {
                if self
                    .outlines
                    .get(file)
                    .is_some_and(|state| !state.filter.is_empty())
                {
                    self.outlines.get_mut(file).filter.set("");
                    self.outlines.get_mut(file).scroll = 0;
                } else if let Some(pane) = self.file_pane_for_outline(scope, file) {
                    self.focus_pane(pane);
                }
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => {
                let step = if event.logical_key == Key::Named(NamedKey::ArrowUp) {
                    -1
                } else {
                    1
                };
                self.step_outline(file, step);
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(index) = self.outlines.get(file).and_then(|state| state.selected) {
                    self.jump_to_outline(scope, index);
                }
            }
            _ => {
                self.outlines
                    .get_mut(file)
                    .filter
                    .press(event, self.modifiers);
                let state = self.outlines.get_mut(file);
                if !state
                    .visible()
                    .iter()
                    .any(|(index, _)| Some(*index) == state.selected)
                {
                    state.selected = state.visible().first().map(|(index, _)| *index);
                }
                state.scroll = 0;
            }
        }
        true
    }

    /// Moves the selected outline row among the rows currently visible.
    fn step_outline(&mut self, file: crate::editor::FileId, step: isize) {
        let state = self.outlines.get_mut(file);
        let rows = state.visible();
        if rows.is_empty() {
            return;
        }
        let current = state
            .selected
            .and_then(|selected| rows.iter().position(|(index, _)| *index == selected));
        let next = current.map_or(0, |index| {
            index.saturating_add_signed(step).min(rows.len() - 1)
        });
        state.selected = Some(rows[next].0);
        let height = self
            .geometry
            .pane_size(self.panes.focus())
            .map_or(400.0, |size| size.height);
        let visible = ((height - 70.0) / outline::ROW_HEIGHT).max(1.0) as usize;
        if next < state.scroll {
            state.scroll = next;
        } else if next >= state.scroll + visible {
            state.scroll = next + 1 - visible;
        }
    }

    /// Moves the followed file's cursor to a selected declaration and focuses its pane.
    fn jump_to_outline(&mut self, scope: Scope, index: usize) {
        let Some(file) = self.outlines.followed(scope) else {
            return;
        };
        let Some(symbol) = self
            .outlines
            .get(file)
            .and_then(|state| state.symbols.get(index))
            .cloned()
        else {
            return;
        };
        self.outlines.get_mut(file).selected = Some(index);
        let Some(pane) = self.file_pane_for_outline(scope, file) else {
            return;
        };
        self.apply(Message::JumpTo(pane, symbol.position));
        if let Some(document) = self.editor.get(file) {
            let mut document = document.borrow_mut();
            let half = document.rows() / 2;
            document.scroll_to(symbol.position.line.saturating_sub(half));
        }
    }

    /// Finds the pane showing `file`, reopening its tab in the last pane that showed it.
    fn file_pane_for_outline(
        &mut self,
        scope: Scope,
        file: crate::editor::FileId,
    ) -> Option<PaneId> {
        let visible = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.active(Some(scope)) == Some(Item::File(file)))
        });
        if visible.is_some() {
            return visible;
        }
        let held = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.items().any(|item| item == Item::File(file)))
        });
        if let Some(pane) = held {
            self.activate_tab(pane, Item::File(file));
            return Some(pane);
        }
        let pane = self
            .outlines
            .pane(file)
            .filter(|pane| self.panes.pane(*pane).is_some())
            .or_else(|| {
                self.panes.panes().into_iter().find(|pane| {
                    self.panes
                        .pane(*pane)
                        .is_some_and(|pane| pane.items().any(|item| item == Item::Outline(scope)))
                })
            })?;
        self.show_file(pane, file, false);
        Some(pane)
    }

    /// Scrolls the outline under the pointer by whole symbol rows.
    pub(super) fn scroll_outline(&mut self, rows: isize) -> bool {
        let Some(pane) = self
            .pointer
            .and_then(|pointer| self.geometry.pane_at(pointer))
        else {
            return false;
        };
        let Some(Item::Outline(scope)) = self
            .panes
            .pane(pane)
            .and_then(|pane| pane.active(self.scope()))
        else {
            return false;
        };
        let Some(file) = self.outlines.followed(scope) else {
            return true;
        };
        let state = self.outlines.get_mut(file);
        let last = state.visible().len().saturating_sub(1);
        state.scroll = state.scroll.saturating_add_signed(rows).min(last);
        true
    }
}

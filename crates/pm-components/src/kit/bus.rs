//! What the user did to a named control, as values the host carries out.
//!
//! There are no closures over the model in the views: a control named with a
//! [`FocusId`] turns presses, drags, hovers, the wheel and dropped files into
//! [`Action`]s, which the kit queues on the thread while the interface handles
//! an event. The host takes them with [`drain`] after handing the event to
//! the interface and carries each out on the model, as it did with the ids it
//! hit-tested before: `Press` is `activate`, `Field` is a press or drag in a
//! text field, `Scroll` scrolls a box by id.

use std::cell::RefCell;
use std::path::PathBuf;

use rustle::{Modifier, PointerButton, PointerEvent, PointerPhase};

use crate::kit::focus::FocusId;
use crate::kit::keys::Modifiers;
use crate::kit::report::{self, Region};

/// Where a drag is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// The button went down.
    Start,
    /// The pointer moved with the button held, wherever it is.
    Move,
    /// The button was let go, wherever the pointer is.
    End,
}

/// Something the user did to a named control. Positions are in logical
/// pixels from the window's top left.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// The primary button went down on `id`: what `pressed_on` recorded, and
    /// what takes focus away from anything else.
    Down {
        /// The control.
        id: FocusId,
        /// Where the pointer is.
        at: (f32, f32),
        /// 1 for a single press, 2 for a double click, 3 for a triple click.
        clicks: u32,
        /// The modifier keys held.
        modifiers: Modifiers,
    },
    /// The primary button went down and up on `id`: carry out what pressing it means.
    Press(FocusId),
    /// The secondary button was pressed and let go on `id`: open its menu at `at`.
    Menu {
        /// The control.
        id: FocusId,
        /// Where the pointer is.
        at: (f32, f32),
    },
    /// A control made draggable was pressed, dragged or let go.
    Drag {
        /// The control.
        id: FocusId,
        /// Where the drag is.
        phase: Phase,
        /// Where the pointer is.
        at: (f32, f32),
    },
    /// The pointer came onto (`inside`) or left a control that follows hovering.
    Hover {
        /// The control.
        id: FocusId,
        /// Whether the pointer is now over it.
        inside: bool,
    },
    /// The scrolling box `id`, which can scroll `room` pixels, was scrolled by
    /// `pixels`, downwards when positive, by the wheel or its bar.
    Scroll {
        /// The box.
        id: FocusId,
        /// How far, in logical pixels.
        pixels: f32,
        /// How far its content can scroll at all.
        room: f32,
    },
    /// The primary button went down in, dragged over, or let go of the text
    /// field `id`, at the shown byte `offset` nearest the pointer.
    Field {
        /// The field.
        id: FocusId,
        /// Where the press is.
        phase: Phase,
        /// The shown byte nearest the pointer.
        offset: usize,
        /// 1 for a single press, 2 for a double click, 3 for a triple click.
        clicks: u32,
        /// Where the pointer is.
        at: (f32, f32),
        /// The modifier keys held.
        modifiers: Modifiers,
    },
    /// A file from another application was dropped on `id`.
    Dropped {
        /// The control or area it was dropped on.
        id: FocusId,
        /// The file.
        path: PathBuf,
    },
}

thread_local! {
    /// The actions taken since the host last drained them.
    static QUEUE: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
}

/// Queues `action` for the host.
pub fn send(action: Action) {
    QUEUE.with(|queue| queue.borrow_mut().push(action));
}

/// Takes every action queued since the last call, oldest first.
pub fn drain() -> Vec<Action> {
    QUEUE.with(|queue| std::mem::take(&mut *queue.borrow_mut()))
}

/// The window position of `event`.
fn at(event: &PointerEvent) -> (f32, f32) {
    (event.window_position.x, event.window_position.y)
}

/// Modifiers that name a control to the host and turn what is done to it into actions.
pub trait Interactive: Sized {
    /// Names the area `id`: it is recorded in the report in tab order, a
    /// primary press on it sends [`Action::Down`] and, released on it,
    /// [`Action::Press`], and a secondary click sends [`Action::Menu`]: `.focusable(id)`.
    fn target(self, id: FocusId) -> Self;
    /// Records the area as `id` in the report without taking presses or focus,
    /// for areas the host measures, such as a scrolling box or a panel.
    fn region(self, id: FocusId) -> Self;
    /// Sends [`Action::Drag`] for presses, drags and releases of the primary button on the area.
    fn draggable_id(self, id: FocusId) -> Self;
    /// Sends [`Action::Hover`] when the pointer comes onto or leaves the area.
    fn hover_id(self, id: FocusId) -> Self;
    /// Sends [`Action::Dropped`] for each file dropped on the area.
    fn file_drop_id(self, id: FocusId) -> Self;
    /// Keeps `ids`, entries of a long list that were not built, in tab order after the regions: `.virtual_focus(ids)`.
    fn virtual_focus(self, ids: Vec<FocusId>) -> Self;
    /// Records the area as one box of the selectable text `id`, such as one
    /// message of a conversation, so the host can tell which box a point is
    /// in: `.selectable(id)` on the box holding the lines.
    fn selectable_area(self, id: FocusId) -> Self;
}

impl Interactive for Modifier {
    fn target(self, id: FocusId) -> Self {
        self.on_placed_visible(move |rect| {
            report::record(|report| {
                report.regions.push(Region {
                    id,
                    rect,
                    focusable: true,
                })
            })
        })
        .on_pointer_button(PointerButton::Primary, move |event| {
            let action = match event.phase {
                PointerPhase::Press => Action::Down {
                    id,
                    at: at(event),
                    clicks: event.click_count,
                    modifiers: Modifiers::from_window(event.modifiers),
                },
                PointerPhase::Click => Action::Press(id),
                PointerPhase::Drag | PointerPhase::Release => return,
            };
            send(action);
        })
        .on_secondary_click(move |event| send(Action::Menu { id, at: at(event) }))
    }

    fn region(self, id: FocusId) -> Self {
        self.on_placed_visible(move |rect| {
            report::record(|report| {
                report.regions.push(Region {
                    id,
                    rect,
                    focusable: false,
                })
            })
        })
    }

    fn draggable_id(self, id: FocusId) -> Self {
        self.on_pointer_button(PointerButton::Primary, move |event| {
            let phase = match event.phase {
                PointerPhase::Press => Phase::Start,
                PointerPhase::Drag => Phase::Move,
                PointerPhase::Release => Phase::End,
                PointerPhase::Click => return,
            };
            send(Action::Drag {
                id,
                phase,
                at: at(event),
            });
        })
    }

    fn hover_id(self, id: FocusId) -> Self {
        self.on_hover(move |inside| send(Action::Hover { id, inside }))
    }

    fn file_drop_id(self, id: FocusId) -> Self {
        self.on_file_drop(move |path| send(Action::Dropped { id, path }))
    }

    fn selectable_area(self, id: FocusId) -> Self {
        self.on_placed_visible(move |rect| {
            report::record(|report| report.selectable_areas.push((id, rect)))
        })
    }

    fn virtual_focus(self, ids: Vec<FocusId>) -> Self {
        self.on_placed(move |_, _| {
            report::record(|report| report.virtual_focus.extend(ids.iter().copied()))
        })
    }
}

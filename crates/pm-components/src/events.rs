//! What the user does to the controls the views draw, handed straight to
//! the app's state holder as it happens.
//!
//! The views are composed under a [`Handler`], the app's state holder as the
//! views see it, provided with [`provide`]. The modifiers of [`Interactive`]
//! read it while composing and close over it, so a press, a click, the wheel
//! or a dropped file calls the holder from inside the interface's handling of
//! the event, the way an event lambda in a composable calls its view model.

use std::path::PathBuf;
use std::rc::Rc;

use rustle::{CompositionLocal, Modifier, PointerButton, PointerEvent, PointerPhase};

use crate::kit::focus::FocusId;
use crate::kit::report::{self, Region};

/// The app's state holder as the views see it: what it does when the user
/// acts on a control. Positions are logical pixels from the window's top left.
pub trait Handler {
    /// The primary button went down on the control `id`.
    fn pressed(&self, id: FocusId);
    /// The primary button went down and came up again on the control `id`.
    fn clicked(&self, id: FocusId);
    /// The secondary button went down on the control `id`.
    fn context_pressed(&self, id: FocusId);
    /// The wheel or a scrollbar moved the scrolling box `id`, which can
    /// scroll `room` pixels in all, by `pixels`, downwards when positive.
    fn scrolled(&self, id: FocusId, pixels: f32, room: f32);
    /// A file from another application was dropped on the area `id`.
    fn dropped(&self, id: FocusId, path: PathBuf);
}

/// A handler that does nothing, for views composed with no app behind them,
/// such as a component's own previews.
struct Unattended;

impl Handler for Unattended {
    /// Does nothing.
    fn pressed(&self, _: FocusId) {}

    /// Does nothing.
    fn clicked(&self, _: FocusId) {}

    /// Does nothing.
    fn context_pressed(&self, _: FocusId) {}

    /// Does nothing.
    fn scrolled(&self, _: FocusId, _: f32, _: f32) {}

    /// Does nothing.
    fn dropped(&self, _: FocusId, _: PathBuf) {}
}

/// A handler as a composition local holds it: equal to another when both
/// are the same handler, so views that skip see whether it changed.
#[derive(Clone)]
struct Provided(Rc<dyn Handler>);

impl PartialEq for Provided {
    /// Whether both are the same handler.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The handler the views composed below are under.
static LOCAL_HANDLER: CompositionLocal<Provided> =
    CompositionLocal::new(|| Provided(Rc::new(Unattended)));

/// Composes `content` under `handler`.
pub fn provide<R>(handler: Rc<dyn Handler>, content: impl FnOnce() -> R) -> R {
    LOCAL_HANDLER.provide(Provided(handler), content)
}

/// The handler the view being composed is under.
pub fn handler() -> Rc<dyn Handler> {
    LOCAL_HANDLER.current().0
}

/// Modifiers that name a control and hand what is done to it to the [`Handler`].
pub trait Interactive: Sized {
    /// Names the area `id`: it is recorded in the report in tab order, a
    /// primary press on it calls [`Handler::pressed`] and, released on it,
    /// [`Handler::clicked`], and a secondary press [`Handler::context_pressed`].
    fn target(self, id: FocusId) -> Self;
    /// Records the area as `id` in the report without taking presses or focus,
    /// for areas the app measures, such as a scrolling box or a panel.
    fn region(self, id: FocusId) -> Self;
    /// Takes presses, drags and releases of the primary button on the area,
    /// so they reach the app as the pointer's own and not a control's
    /// beneath, and calls [`Handler::pressed`] when the button goes down.
    fn draggable_id(self, id: FocusId) -> Self;
    /// Calls [`Handler::dropped`] for each file dropped on the area.
    fn file_drop_id(self, id: FocusId) -> Self;
    /// Keeps `ids`, entries of a long list that were not built, in tab order after the regions.
    fn virtual_focus(self, ids: Vec<FocusId>) -> Self;
    /// Records the area as one box of the selectable text `id`, such as one
    /// message of a conversation, so the app can tell which box a point is in.
    fn selectable_area(self, id: FocusId) -> Self;
}

impl Interactive for Modifier {
    fn target(self, id: FocusId) -> Self {
        let handler = handler();
        let context = Rc::clone(&handler);
        self.on_placed_visible(move |rect| {
            report::record(|report| {
                report.regions.push(Region {
                    id,
                    rect,
                    focusable: true,
                })
            })
        })
        .on_pointer_button(PointerButton::Primary, move |event| match event.phase {
            PointerPhase::Press => handler.pressed(id),
            PointerPhase::Click => handler.clicked(id),
            PointerPhase::Drag | PointerPhase::Release => {}
        })
        .on_pointer_button(PointerButton::Secondary, move |event| {
            if event.phase == PointerPhase::Press {
                context.context_pressed(id);
            }
        })
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
        let handler = handler();
        self.on_pointer_button(PointerButton::Primary, move |event: &PointerEvent| {
            if event.phase == PointerPhase::Press {
                handler.pressed(id);
            }
        })
    }

    fn file_drop_id(self, id: FocusId) -> Self {
        let handler = handler();
        self.on_file_drop(move |path| handler.dropped(id, path))
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

//! Where an area was drawn, held by the state that owns the area, as a
//! Compose state holder keeps what `onGloballyPositioned` told it.

use crate::target::Target;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use rustle::{Modifier, Rect};

/// Where an area is drawn in its window, in logical pixels: told by the area
/// each time the display is built. Two handles are equal when they are the
/// same holder.
#[derive(Clone, Default)]
pub struct Bounds(Rc<Held>);

/// What a [`Bounds`] holds.
#[derive(Default)]
struct Held {
    /// Where the area is drawn while it is in the composition.
    shown: Cell<Option<Rect>>,
    /// Where the area was last drawn, kept after it left the composition.
    last: Cell<Option<Rect>>,
}

impl Bounds {
    /// Where the area is drawn, or nothing while it is not in the composition.
    pub fn get(&self) -> Option<Rect> {
        self.0.shown.get()
    }

    /// Where the area was last drawn, even when it has left the composition
    /// since, such as when the composition that drew it was a single frame
    /// drawn on its own.
    pub fn last(&self) -> Option<Rect> {
        self.0.last.get()
    }

    /// `modifier` telling these bounds where its area is drawn, each time the
    /// display is built; once the call leaves the composition, [`get`](Self::get)
    /// answers nothing again.
    #[track_caller]
    pub fn track(&self, modifier: Modifier) -> Modifier {
        modifier.on_globally_positioned(self.told())
    }

    /// Like [`track`](Self::track), telling the part of the area the clips around it leave
    /// visible, an empty rectangle while it is scrolled or clipped out of sight: for areas the
    /// app finds under a point.
    pub fn track_visible(&self, modifier: Modifier) -> Modifier {
        modifier.on_placed_visible(self.told())
    }

    /// What tells these bounds where their area is drawn, forgetting it once the call leaves
    /// the composition.
    #[track_caller]
    fn told(&self) -> impl Fn(Rect) + 'static {
        let gone = self.0.clone();
        rustle::disposable_effect((), move || move || gone.shown.set(None));
        let seen = self.0.clone();
        move |rect| {
            seen.shown.set(Some(rect));
            seen.last.set(Some(rect));
        }
    }
}

impl PartialEq for Bounds {
    /// Whether both handles are the same holder.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Bounds {
    /// Where the area is drawn and where it was last drawn.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Bounds")
            .field("shown", &self.get())
            .field("last", &self.last())
            .finish()
    }
}

/// Where the areas the app asks about by name are drawn: the [`Bounds`] of each control
/// that tracks itself with [`placed`], held by the state that owns the screen and provided
/// to the views with [`Places::provide`]. Two handles are equal when they are the same
/// holder.
#[derive(Clone, Default)]
pub struct Places(Rc<std::cell::RefCell<std::collections::HashMap<Target, Bounds>>>);

/// The places the views composed below report to.
static LOCAL_PLACES: rustle::CompositionLocal<Option<Places>> =
    rustle::CompositionLocal::new(|| None);

impl Places {
    /// The bounds of the area named `id`, made when first asked for.
    pub fn of(&self, id: Target) -> Bounds {
        self.0.borrow_mut().entry(id).or_default().clone()
    }

    /// Where the area named `id` is drawn, while it is in the composition.
    pub fn get(&self, id: Target) -> Option<Rect> {
        self.0.borrow().get(&id).and_then(Bounds::get)
    }

    /// The areas shown now and where they are drawn, in no particular order.
    pub fn shown(&self) -> Vec<(Target, Rect)> {
        self.0
            .borrow()
            .iter()
            .filter_map(|(id, bounds)| Some((*id, bounds.get()?)))
            .collect()
    }

    /// Composes `content` with the areas in it that track themselves reporting here.
    pub fn provide<R>(&self, content: impl FnOnce() -> R) -> R {
        LOCAL_PLACES.provide(Some(self.clone()), content)
    }
}

impl PartialEq for Places {
    /// Whether both handles are the same holder.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Places {
    /// How many areas are tracked.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Places")
            .field("areas", &self.0.borrow().len())
            .finish()
    }
}

/// `modifier` telling the [`Places`] the views are composed with the part of the area named
/// `id` that is visible, as `onGloballyPositioned` tells a state holder; nothing without one.
#[track_caller]
pub fn placed(id: Target, modifier: Modifier) -> Modifier {
    match LOCAL_PLACES.current() {
        Some(places) => places.of(id).track_visible(modifier),
        None => modifier,
    }
}

//! The revision of what the screen shows, so a region of the screen is composed again only
//! when what it shows or the room it is given changed.
//!
//! The whole screen is one state that the root reads, so any change composes the root again.
//! The host counts, as the revision, every change of the screen but not a change of the size
//! of a panel: dragging the handle between two panels moves only sizes. A region composed by [`region`] is keyed by the revision and by
//! the sizes it uses, so while a handle is dragged the regions whose room did not change are
//! not composed again, and those that read the dragged size are.

use rustle::CompositionLocal;

/// The revision of the screen the view being composed shows.
static LOCAL_REVISION: CompositionLocal<u64> = CompositionLocal::new(|| 0);

/// Composes `content` showing the screen at `revision`.
pub fn provide<R>(revision: u64, content: impl FnOnce() -> R) -> R {
    LOCAL_REVISION.provide(revision, content)
}

/// Composes `content`, a region of the screen given the room `room` (the sizes it uses, such
/// as its own width and the window's), again only when the screen's revision or `room`
/// changed, or state it read did; otherwise what it emitted last time is emitted again.
///
/// `content` must take from the screen only what the revision counts, and from the sizes of
/// panels only what is in `room`.
#[track_caller]
pub fn region<D: PartialEq + 'static>(room: D, content: impl FnOnce()) {
    let revision = LOCAL_REVISION.current();
    rustle::skippable((revision, room), content);
}

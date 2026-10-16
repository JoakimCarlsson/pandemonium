//! How big the interface is built: the share of its comfortable room each spacing step takes, and how much bigger than it was drawn for its text is, set for the thread before a frame is built.

use std::cell::Cell;

thread_local! {
    /// The scale of a spacing step and the scale of interface text, for the frame being built on this thread.
    static SCALES: Cell<(f32, f32)> = const { Cell::new((1.0, 1.0)) };
}

/// Builds what follows on this thread with each spacing step `step` of its comfortable room and interface text `text` times as big as it was drawn for.
pub fn set_scales(step: f32, text: f32) {
    SCALES.with(|scales| scales.set((step, text)));
}

/// The share of its comfortable room a spacing step takes on this thread.
pub fn step_scale() -> f32 {
    SCALES.with(|scales| scales.get().0)
}

/// How much bigger than it was drawn for interface text is on this thread.
pub fn text_scale() -> f32 {
    SCALES.with(|scales| scales.get().1)
}

//! Content positions, pointer selection and selectable reading surfaces.

mod model;
mod surface;

pub use model::{Grain, Selection, SelectionContent, SelectionDrag, SelectionRow, Spot, spot_at};
pub(crate) use surface::{SelectionFrame, SelectionRegistry, SelectionSurface};

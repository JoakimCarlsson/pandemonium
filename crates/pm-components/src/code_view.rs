//! The size the view of the file shown is laid out at, held by the workspace's state the
//! way a lazy list's state holds what its layout found.

use std::fmt;
use std::rc::Rc;

use rustle::{Modifier, Size};

/// The size the view of the file shown was last laid out at, in logical pixels, told by the
/// view itself as Compose's `onSizeChanged` tells a state holder, and kept in rustle state so
/// what reads it, such as soft wrap, is composed again when it changes. Nothing until the view
/// was laid out. Two handles are equal when they are the same holder.
#[derive(Clone)]
pub struct CodeViewSize(Rc<rustle::State<Option<Size>>>);

impl Default for CodeViewSize {
    /// A holder that knows no size yet.
    fn default() -> Self {
        Self(Rc::new(rustle::mutable_state(None)))
    }
}

impl CodeViewSize {
    /// The size the view was last laid out at, or nothing before it was.
    pub fn get(&self) -> Option<Size> {
        self.0.get()
    }

    /// Holds `size` as the view's, changing the state only when it differs from what it held.
    pub fn set(&self, size: Size) {
        if self.0.get() != Some(size) {
            self.0.set(Some(size));
        }
    }

    /// `modifier` telling this holder the size its area is laid out at each time that changes.
    pub fn track(&self, modifier: Modifier) -> Modifier {
        let held = self.clone();
        modifier.on_size_changed(move |size| held.set(size))
    }
}

impl PartialEq for CodeViewSize {
    /// Whether both handles are the same holder.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for CodeViewSize {
    /// The size held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CodeViewSize")
            .field(&self.get())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every handle sees the size told to one, and handles are equal only to their own holder.
    #[test]
    fn handles_share_the_size_told() {
        let held = CodeViewSize::default();
        let other = held.clone();
        assert_eq!(other.get(), None);
        held.set(Size::new(640.0, 480.0));
        assert_eq!(other.get(), Some(Size::new(640.0, 480.0)));
        assert_eq!(held, other);
        assert_ne!(held, CodeViewSize::default());
    }
}

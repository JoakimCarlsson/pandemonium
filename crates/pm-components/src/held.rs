//! A value held as observable state: the composables that read it are composed, laid
//! out or drawn again when it changes, without the state that owns it being written.
//!
//! Panel sizes are held this way, so dragging a handle between panels changes only the
//! size it drags, and only what reads that size follows. Each holder is the one source of
//! truth of its value: a clone is a new holder with the same value, never a second handle
//! to the first, so a layout kept aside for another session keeps its own sizes.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A value of type `T` held as observable state.
pub struct Held<T: Clone + 'static>(rustle::State<T>);

impl<T: Clone + PartialEq + 'static> Held<T> {
    /// A holder of `value`.
    pub fn new(value: T) -> Self {
        Self(rustle::mutable_state(value))
    }

    /// The value held, read as observable state.
    pub fn get(&self) -> T {
        self.0.get()
    }

    /// Holds `value`, telling its readers only when it differs from what was held.
    pub fn set(&self, value: T) {
        if self.0.with(|held| *held != value) {
            self.0.set(value);
        }
    }
}

impl<T: Clone + PartialEq + 'static> Clone for Held<T> {
    /// A new holder of the same value.
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl<T: Clone + PartialEq + 'static> PartialEq for Held<T> {
    /// Whether both hold equal values.
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: Clone + PartialEq + Default + 'static> Default for Held<T> {
    /// A holder of the default value.
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone + PartialEq + fmt::Debug + 'static> fmt::Debug for Held<T> {
    /// Writes the value held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

impl<T: Clone + PartialEq + Serialize + 'static> Serialize for Held<T> {
    /// Writes the value held.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.with(|held| held.serialize(serializer))
    }
}

impl<'de, T: Clone + PartialEq + Deserialize<'de> + 'static> Deserialize<'de> for Held<T> {
    /// A holder of the value read.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::new)
    }
}

impl<T: Clone + PartialEq + 'static> From<T> for Held<T> {
    /// A holder of `value`.
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clone holds the same value but is a holder of its own.
    #[test]
    fn a_clone_is_a_holder_of_its_own() {
        let held = Held::new(1.0_f32);
        let copy = held.clone();
        assert_eq!(held, copy);
        copy.set(2.0);
        assert_eq!(held.get(), 1.0);
        assert_ne!(held, copy);
    }
}

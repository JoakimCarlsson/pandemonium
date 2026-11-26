//! A list of paths that carries a generation, which changes whenever the list may have, so
//! what is worked out from it can be kept by comparing a number rather than every path.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

/// The generation the next list made or changed is given.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// A generation no list has had before.
fn fresh() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Paths, read as a `Vec<String>`, with a generation that moves on every change: two lists
/// of the same generation hold the same paths.
#[derive(Clone, Debug)]
pub struct Listed {
    /// The paths.
    items: Vec<String>,
    /// The generation, given anew each time the paths may have changed.
    generation: u64,
}

impl Listed {
    /// The generation of the paths: the same for as long as they are not changed.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Holds `items` from now on, keeping the generation when they are the paths already held.
    pub fn set(&mut self, items: Vec<String>) {
        if self.items != items {
            *self = items.into();
        }
    }
}

impl Default for Listed {
    /// No paths.
    fn default() -> Self {
        Vec::new().into()
    }
}

impl From<Vec<String>> for Listed {
    /// `items`, given a generation of their own.
    fn from(items: Vec<String>) -> Self {
        Self {
            items,
            generation: fresh(),
        }
    }
}

impl FromIterator<String> for Listed {
    /// The paths `items` yields, given a generation of their own.
    fn from_iter<I: IntoIterator<Item = String>>(items: I) -> Self {
        items.into_iter().collect::<Vec<_>>().into()
    }
}

impl PartialEq for Listed {
    /// Whether both lists hold the same paths.
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation || self.items == other.items
    }
}

impl Deref for Listed {
    type Target = Vec<String>;

    /// The paths.
    fn deref(&self) -> &Vec<String> {
        &self.items
    }
}

impl DerefMut for Listed {
    /// The paths to change, which moves the generation on.
    fn deref_mut(&mut self) -> &mut Vec<String> {
        self.generation = fresh();
        &mut self.items
    }
}

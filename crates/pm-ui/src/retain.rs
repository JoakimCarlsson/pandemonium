//! Subtrees built and laid out once, then kept across frames for as long as
//! what they show stays the same.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use pm_style::layout::{Placed, fit};
use pm_text::Shaper;

use crate::node::Node;

/// How many subtrees the newer generation of the cache holds at the end of a
/// frame before it becomes the older one and the oldest are let go.
const GENERATION: usize = 256;

/// What a kept subtree is found by: the caller's key and the width as bits.
type Key = (u64, u32);

/// A subtree laid out at a fixed width, and where each of its boxes ended up.
#[derive(Debug)]
pub(crate) struct Retained {
    /// The subtree.
    pub(crate) root: Node,
    /// Its boxes, the root's top-left corner at the origin.
    pub(crate) placed: Placed,
}

/// The subtrees drawn lately, in two generations: one found in the older is
/// moved to the newer, and the older is let go when the newer has filled by
/// the end of a frame, so a subtree drawn every frame is never built again.
#[derive(Default)]
struct Cache {
    /// The subtrees built or found since the generations last turned over.
    newer: HashMap<Key, Arc<Retained>>,
    /// The subtrees from the generation before.
    older: HashMap<Key, Arc<Retained>>,
}

impl Cache {
    /// The subtree kept under `key`, moved to the newer generation.
    fn find(&mut self, key: Key) -> Option<Arc<Retained>> {
        if let Some(found) = self.newer.get(&key) {
            return Some(found.clone());
        }
        let found = self.older.remove(&key)?;
        self.keep(key, found.clone());
        Some(found)
    }

    /// Keeps `retained` under `key`.
    fn keep(&mut self, key: Key, retained: Arc<Retained>) {
        self.newer.insert(key, retained);
    }

    /// Lets the older generation go and starts a new one when the newer is full.
    fn turn_over(&mut self) {
        if self.newer.len() >= GENERATION {
            self.older = std::mem::take(&mut self.newer);
        }
    }
}

thread_local! {
    /// The subtrees kept on this thread.
    static CACHE: RefCell<Cache> = RefCell::default();
}

/// A box holding the subtree `build` makes, laid out `width` logical pixels
/// wide and as tall as it comes out.
///
/// The subtree is built and laid out the first time `key` is asked for at
/// `width`, and the same one is handed back while it keeps being drawn, so
/// neither its building nor its layout is paid again. `key` must change
/// whenever what `build` makes would. Its boxes still answer to the pointer
/// and take focus as any others do.
pub fn retained(
    key: u64,
    width: f32,
    shaper: &mut Shaper,
    build: impl FnOnce(&mut Shaper) -> Node,
) -> Node {
    let key = (key, width.to_bits());
    let found = CACHE.with_borrow_mut(|cache| cache.find(key));
    let retained = found.unwrap_or_else(|| {
        let root = build(shaper);
        let placed = fit(&root.item(shaper), width);
        let made = Arc::new(Retained { root, placed });
        CACHE.with_borrow_mut(|cache| cache.keep(key, made.clone()));
        made
    });
    Node {
        retained: Some(retained),
        ..Node::default()
    }
}

/// Ends a frame for the kept subtrees: those not drawn for a while are let go.
pub(crate) fn end_frame() {
    CACHE.with_borrow_mut(Cache::turn_over);
}

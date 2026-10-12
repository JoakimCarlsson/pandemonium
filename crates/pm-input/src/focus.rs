//! Which element has keyboard focus, and the order Tab walks them in.

/// Names one element that can take focus.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FocusId(pub u64);

/// The focus order of one screen and the element holding focus in it.
#[derive(Clone, Debug, Default)]
pub struct Focus {
    /// The focusable elements, in tab order.
    order: Vec<FocusId>,
    /// The element that holds focus, if any.
    current: Option<FocusId>,
}

impl Focus {
    /// An empty order with nothing focused.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the tab order, dropping focus if its holder is no longer in it.
    pub fn set_order(&mut self, order: Vec<FocusId>) {
        self.order = order;
        if self
            .current
            .is_some_and(|current| !self.order.contains(&current))
        {
            self.current = None;
        }
    }

    /// The element holding focus.
    pub fn current(&self) -> Option<FocusId> {
        self.current
    }

    /// Whether `id` holds focus.
    pub fn is_focused(&self, id: FocusId) -> bool {
        self.current == Some(id)
    }

    /// Gives focus to `id`, if it is in the order.
    pub fn focus(&mut self, id: FocusId) {
        if self.order.contains(&id) {
            self.current = Some(id);
        }
    }

    /// Takes focus away from whatever holds it.
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// Moves focus to the next element, wrapping round to the first.
    pub fn next(&mut self) {
        self.step(1);
    }

    /// Moves focus to the previous element, wrapping round to the last.
    pub fn previous(&mut self) {
        self.step(self.order.len().saturating_sub(1));
    }

    /// Moves focus `by` places along the order, wrapping round.
    fn step(&mut self, by: usize) {
        if self.order.is_empty() {
            return;
        }
        let position = self
            .current
            .and_then(|current| self.order.iter().position(|id| *id == current));
        let next = match position {
            Some(position) => (position + by) % self.order.len(),
            None if by == 1 => 0,
            None => self.order.len() - 1,
        };
        self.current = Some(self.order[next]);
    }
}

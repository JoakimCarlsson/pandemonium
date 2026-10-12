//! The conditions under which a utility applies.

use pm_layout::Breakpoint;

/// A condition an element can be in.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum State {
    /// The pointer is over the element.
    Hover,
    /// The element has keyboard focus.
    Focus,
    /// The pointer is held down on the element.
    Active,
    /// The element does not take input.
    Disabled,
    /// The window uses the dark appearance.
    Dark,
}

impl State {
    /// The bit this state occupies in a [`StateSet`].
    const fn bit(self) -> u8 {
        match self {
            Self::Hover => 1,
            Self::Focus => 1 << 1,
            Self::Active => 1 << 2,
            Self::Disabled => 1 << 3,
            Self::Dark => 1 << 4,
        }
    }
}

/// A set of [`State`]s.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StateSet(u8);

impl StateSet {
    /// The set holding no state.
    pub const EMPTY: Self = Self(0);

    /// The same set with `state` added.
    pub const fn with(self, state: State) -> Self {
        Self(self.0 | state.bit())
    }

    /// Whether `state` is in the set.
    pub const fn contains(self, state: State) -> bool {
        self.0 & state.bit() != 0
    }

    /// Whether every state of `other` is in this set.
    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether the set holds no state.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// What a style is resolved against: the element's states and the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Context {
    /// The states the element is in.
    pub states: StateSet,
    /// The viewport width in logical pixels.
    pub viewport_width: f32,
}

/// The condition gating a utility: states that must all hold and a smallest viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Variants {
    /// The states the element must all be in.
    pub states: StateSet,
    /// The breakpoint the viewport must have reached.
    pub breakpoint: Option<Breakpoint>,
}

impl Variants {
    /// No condition: the utility always applies.
    pub const ALWAYS: Self = Self {
        states: StateSet::EMPTY,
        breakpoint: None,
    };

    /// Whether the utility applies in `context`.
    pub fn matches(self, context: &Context) -> bool {
        context.states.contains_all(self.states)
            && self
                .breakpoint
                .is_none_or(|point| context.viewport_width >= point.min_width())
    }

    /// Whether the utility always applies.
    pub fn is_always(self) -> bool {
        self.states.is_empty() && self.breakpoint.is_none()
    }

    /// The smallest viewport width at which the condition can hold.
    pub fn min_width(self) -> f32 {
        self.breakpoint.map_or(0.0, Breakpoint::min_width)
    }
}

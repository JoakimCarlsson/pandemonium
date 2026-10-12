//! Flex containers and flex items.

use pm_primitives::{Length, Rule};

/// The main axis of a flex container and the direction along it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FlexDirection {
    /// Left to right.
    #[default]
    Row,
    /// Right to left.
    RowReverse,
    /// Top to bottom.
    Column,
    /// Bottom to top.
    ColumnReverse,
}

/// Whether and how a flex container breaks into lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FlexWrap {
    /// One line, whatever the content comes to.
    #[default]
    NoWrap,
    /// New lines are added after the last one.
    Wrap,
    /// New lines are added before the first one.
    WrapReverse,
}

/// How free space is distributed along the main axis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum JustifyContent {
    /// The container's default packing.
    #[default]
    Normal,
    /// Packed at the start.
    Start,
    /// Packed at the end.
    End,
    /// Packed in the middle.
    Center,
    /// The first and last at the edges, the rest evenly between.
    Between,
    /// Equal space around every item.
    Around,
    /// Equal space between and around every item.
    Evenly,
    /// Items grow to fill the line.
    Stretch,
}

/// How items sit across the cross axis within a line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AlignItems {
    /// Stretched to the line, unless the item has a cross size.
    #[default]
    Stretch,
    /// At the cross start.
    Start,
    /// At the cross end.
    End,
    /// Centred on the cross axis.
    Center,
    /// Lined up on their text baselines.
    Baseline,
}

/// How lines are distributed along the cross axis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AlignContent {
    /// The container's default packing.
    #[default]
    Normal,
    /// Packed at the cross start.
    Start,
    /// Packed at the cross end.
    End,
    /// Packed in the middle.
    Center,
    /// The first and last lines at the edges, the rest evenly between.
    Between,
    /// Equal space around every line.
    Around,
    /// Equal space between and around every line.
    Evenly,
    /// Lines grow to fill the container.
    Stretch,
}

/// The container half of flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlexStyle {
    /// The main axis.
    pub direction: FlexDirection,
    /// Line breaking.
    pub wrap: FlexWrap,
    /// Distribution along the main axis.
    pub justify_content: JustifyContent,
    /// Alignment of items across a line.
    pub align_items: AlignItems,
    /// Distribution of lines along the cross axis.
    pub align_content: AlignContent,
}

/// The item half of flex layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItem {
    /// Share of the leftover space the item takes.
    pub grow: f32,
    /// Share of the overflow the item gives back.
    pub shrink: f32,
    /// The size along the main axis before growing and shrinking.
    pub basis: Length,
    /// The position in the order of siblings; lower comes first.
    pub order: i32,
    /// This item's own cross-axis alignment, overriding the container's.
    pub align_self: Option<AlignItems>,
}

impl Default for FlexItem {
    /// Does not grow, shrinks with its siblings, sized by content.
    fn default() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: Length::Auto,
            order: 0,
            align_self: None,
        }
    }
}

/// One declaration about flex layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexRule {
    /// The main axis.
    Direction(FlexDirection),
    /// Line breaking.
    Wrap(FlexWrap),
    /// Distribution along the main axis.
    JustifyContent(JustifyContent),
    /// Alignment across a line.
    AlignItems(AlignItems),
    /// Distribution of lines.
    AlignContent(AlignContent),
    /// Growth factor.
    Grow(f32),
    /// Shrink factor.
    Shrink(f32),
    /// Initial main size.
    Basis(Length),
    /// Position among siblings.
    Order(i32),
    /// This item's own alignment.
    AlignSelf(Option<AlignItems>),
    /// Grows and shrinks as needed, ignoring content size.
    Flex1,
    /// Grows and shrinks, but never below content size.
    FlexAuto,
    /// Shrinks, but never grows.
    FlexInitial,
    /// Neither grows nor shrinks.
    FlexNone,
}

impl Rule for FlexRule {
    type Target = (FlexStyle, FlexItem);

    /// Writes the declaration into `target`.
    fn apply(self, (container, item): &mut (FlexStyle, FlexItem)) {
        match self {
            Self::Direction(direction) => container.direction = direction,
            Self::Wrap(wrap) => container.wrap = wrap,
            Self::JustifyContent(justify) => container.justify_content = justify,
            Self::AlignItems(align) => container.align_items = align,
            Self::AlignContent(align) => container.align_content = align,
            Self::Grow(grow) => item.grow = grow,
            Self::Shrink(shrink) => item.shrink = shrink,
            Self::Basis(basis) => item.basis = basis,
            Self::Order(order) => item.order = order,
            Self::AlignSelf(align) => item.align_self = align,
            Self::Flex1 => {
                item.grow = 1.0;
                item.shrink = 1.0;
                item.basis = Length::Fraction(0.0);
            }
            Self::FlexAuto => {
                item.grow = 1.0;
                item.shrink = 1.0;
                item.basis = Length::Auto;
            }
            Self::FlexInitial => {
                item.grow = 0.0;
                item.shrink = 1.0;
                item.basis = Length::Auto;
            }
            Self::FlexNone => {
                item.grow = 0.0;
                item.shrink = 0.0;
                item.basis = Length::Auto;
            }
        }
    }
}

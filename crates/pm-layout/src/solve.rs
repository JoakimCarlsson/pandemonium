//! Solving a tree of boxes: flex and grid layout, from the styles in this crate.
//!
//! The algorithms are `taffy`'s; this module is the one place that knows it.
//! Callers describe boxes with a [`LayoutStyle`] each and get rectangles back.

use pm_primitives::{Edges, Length};
use taffy::prelude::*;

use crate::{
    AlignContent as Content, AlignItems as Items, AutoFlow, Display, FlexDirection, FlexWrap,
    GridLine, JustifyContent, LayoutStyle, Margin, Overflow, Position, Track, TrackList,
};

/// One box to lay out, and the boxes inside it.
#[derive(Clone, Debug, Default)]
pub struct Item {
    /// How the box is sized, spaced and arranged.
    pub style: LayoutStyle,
    /// The widths of the box's borders, which take room inside it.
    pub border: Edges<f32>,
    /// The size the content of a leaf asks for, when it has content that is
    /// not made of boxes, such as a line of text: `(width, height)`.
    pub intrinsic: Option<(f32, f32)>,
    /// The boxes inside it, in layout order.
    pub children: Vec<Item>,
}

/// The solved box of one item and of the items inside it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placed {
    /// The left edge, in the coordinates of the root.
    pub x: f32,
    /// The top edge, in the coordinates of the root.
    pub y: f32,
    /// The extent along x.
    pub width: f32,
    /// The extent along y.
    pub height: f32,
    /// The solved boxes of the item's children, in the same order.
    pub children: Vec<Placed>,
}

/// Lays `root` out inside a viewport of `width` by `height` logical pixels;
/// the root fills it.
pub fn solve(root: &Item, width: f32, height: f32) -> Placed {
    let mut tree: TaffyTree<(f32, f32)> = TaffyTree::new();
    let id = build(&mut tree, root, Some((width, height)));
    let _ = tree.compute_layout_with_measure(
        id,
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        },
        |input, _id, content, style| {
            taffy::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, _available| {
                    let (width, height) = content.map_or((0.0, 0.0), |content| *content);
                    Size {
                        width: known.width.unwrap_or(width),
                        height: known.height.unwrap_or(height),
                    }
                },
            )
        },
    );
    collect(&tree, id, 0.0, 0.0)
}

/// Adds `item` and its children to `tree`, the root pinned to the viewport.
fn build(tree: &mut TaffyTree<(f32, f32)>, item: &Item, viewport: Option<(f32, f32)>) -> NodeId {
    let children: Vec<NodeId> = item
        .children
        .iter()
        .map(|child| build(tree, child, None))
        .collect();
    let mut style = convert(&item.style, item.border);
    if let Some((width, height)) = viewport {
        style.size = Size {
            width: length(width),
            height: length(height),
        };
    }
    let built = match item.intrinsic {
        Some(intrinsic) if children.is_empty() => tree.new_leaf_with_context(style, intrinsic),
        _ => tree.new_with_children(style, &children),
    };
    built.unwrap_or_else(|_| tree.new_leaf(Style::default()).unwrap_or(NodeId::new(0)))
}

/// Reads the solved boxes of `id` and its children, offset by their parents.
fn collect(tree: &TaffyTree<(f32, f32)>, id: NodeId, x: f32, y: f32) -> Placed {
    let layout = tree.layout(id).copied().unwrap_or_default();
    let (x, y) = (x + layout.location.x, y + layout.location.y);
    let children = tree
        .children(id)
        .unwrap_or_default()
        .into_iter()
        .map(|child| collect(tree, child, x, y))
        .collect();
    Placed {
        x,
        y,
        width: layout.size.width,
        height: layout.size.height,
        children,
    }
}

/// A taffy dimension from a style length.
fn dimension(length: Length) -> Dimension {
    match length {
        Length::Px(pixels) => taffy::style_helpers::length(pixels),
        Length::Fraction(fraction) => taffy::style_helpers::percent(fraction),
        Length::Screen => taffy::style_helpers::percent(1.0),
        Length::Auto | Length::MinContent | Length::MaxContent | Length::FitContent => auto(),
    }
}

/// A taffy length-or-auto from a style length.
fn offset(length: Length) -> LengthPercentageAuto {
    match length {
        Length::Px(pixels) => taffy::style_helpers::length(pixels),
        Length::Fraction(fraction) => taffy::style_helpers::percent(fraction),
        Length::Screen => taffy::style_helpers::percent(1.0),
        Length::Auto | Length::MinContent | Length::MaxContent | Length::FitContent => auto(),
    }
}

/// A taffy margin from a style margin.
fn margin(margin: Margin) -> LengthPercentageAuto {
    match margin {
        Margin::Px(pixels) => taffy::style_helpers::length(pixels),
        Margin::Auto => auto(),
    }
}

/// A taffy grid track from a style track.
fn track(track: Track) -> TrackSizingFunction {
    match track {
        Track::Auto => auto(),
        Track::Fr(share) => fr(share),
        Track::Px(pixels) => taffy::style_helpers::length(pixels),
        Track::MinContent => min_content(),
        Track::MaxContent => max_content(),
        Track::MinMax => minmax(min_content(), fr(1.0)),
    }
}

/// A taffy track template from a style track list.
fn template(list: TrackList) -> Vec<GridTemplateComponent<String>> {
    match list {
        TrackList::None | TrackList::Subgrid => Vec::new(),
        TrackList::Repeat(count, one) => vec![repeat(count, vec![track(one)])],
    }
}

/// A taffy grid placement from a style grid line.
fn placement(line: GridLine) -> GridPlacement<String> {
    match line {
        GridLine::Auto => GridPlacement::Auto,
        GridLine::Line(index) => taffy::style_helpers::line(index),
        GridLine::Span(count) => taffy::style_helpers::span(count),
    }
}

/// A taffy item alignment from a style one.
fn items(align: Items) -> AlignItems {
    match align {
        Items::Stretch => AlignItems::STRETCH,
        Items::Start => AlignItems::FLEX_START,
        Items::End => AlignItems::FLEX_END,
        Items::Center => AlignItems::CENTER,
        Items::Baseline => AlignItems::BASELINE,
    }
}

/// A taffy content distribution from a style one.
fn content(align: Content) -> AlignContent {
    match align {
        Content::Normal => AlignContent::NORMAL,
        Content::Start => AlignContent::FLEX_START,
        Content::End => AlignContent::FLEX_END,
        Content::Center => AlignContent::CENTER,
        Content::Between => AlignContent::SPACE_BETWEEN,
        Content::Around => AlignContent::SPACE_AROUND,
        Content::Evenly => AlignContent::SPACE_EVENLY,
        Content::Stretch => AlignContent::STRETCH,
    }
}

/// A taffy main-axis distribution from a style one.
fn justify(justify: JustifyContent) -> AlignContent {
    match justify {
        JustifyContent::Normal => AlignContent::NORMAL,
        JustifyContent::Start => AlignContent::FLEX_START,
        JustifyContent::End => AlignContent::FLEX_END,
        JustifyContent::Center => AlignContent::CENTER,
        JustifyContent::Between => AlignContent::SPACE_BETWEEN,
        JustifyContent::Around => AlignContent::SPACE_AROUND,
        JustifyContent::Evenly => AlignContent::SPACE_EVENLY,
        JustifyContent::Stretch => AlignContent::STRETCH,
    }
}

/// A taffy overflow from a style one.
fn overflow(overflow: Overflow) -> taffy::Overflow {
    match overflow {
        Overflow::Hidden | Overflow::Clip => taffy::Overflow::Hidden,
        Overflow::Visible | Overflow::Scroll | Overflow::Auto => taffy::Overflow::Visible,
    }
}

/// The taffy style that lays out what `layout` describes, with `border` as
/// the widths of the node's borders.
fn convert(layout: &LayoutStyle, border: Edges<f32>) -> Style {
    let spacing = layout.spacing;
    let sizing = layout.sizing;
    let positioning = layout.positioning;
    let flex = layout.flex;
    let item = layout.flex_item;
    let grid = layout.grid;
    let cell = layout.grid_item;
    Style {
        display: match layout.display.display {
            Display::None => taffy::Display::None,
            Display::Flex | Display::InlineFlex => taffy::Display::Flex,
            Display::Grid | Display::InlineGrid => taffy::Display::Grid,
            Display::Block | Display::Inline | Display::InlineBlock | Display::Contents => {
                taffy::Display::Block
            }
        },
        overflow: taffy::Point {
            x: overflow(layout.display.overflow_x),
            y: overflow(layout.display.overflow_y),
        },
        position: match positioning.position {
            Position::Absolute | Position::Fixed => taffy::Position::Absolute,
            Position::Static | Position::Relative | Position::Sticky => taffy::Position::Relative,
        },
        inset: taffy::Rect {
            left: offset(positioning.inset.left),
            right: offset(positioning.inset.right),
            top: offset(positioning.inset.top),
            bottom: offset(positioning.inset.bottom),
        },
        size: Size {
            width: dimension(sizing.width),
            height: dimension(sizing.height),
        },
        min_size: Size {
            width: offset(sizing.min_width),
            height: offset(sizing.min_height),
        },
        max_size: Size {
            width: offset(sizing.max_width),
            height: offset(sizing.max_height),
        },
        aspect_ratio: sizing.aspect_ratio,
        margin: taffy::Rect {
            left: margin(spacing.margin.left),
            right: margin(spacing.margin.right),
            top: margin(spacing.margin.top),
            bottom: margin(spacing.margin.bottom),
        },
        padding: taffy::Rect {
            left: taffy::style_helpers::length(spacing.padding.left),
            right: taffy::style_helpers::length(spacing.padding.right),
            top: taffy::style_helpers::length(spacing.padding.top),
            bottom: taffy::style_helpers::length(spacing.padding.bottom),
        },
        border: taffy::Rect {
            left: taffy::style_helpers::length(border.left),
            right: taffy::style_helpers::length(border.right),
            top: taffy::style_helpers::length(border.top),
            bottom: taffy::style_helpers::length(border.bottom),
        },
        gap: Size {
            width: taffy::style_helpers::length(spacing.column_gap),
            height: taffy::style_helpers::length(spacing.row_gap),
        },
        align_items: items(flex.align_items),
        align_content: content(flex.align_content),
        justify_content: justify(flex.justify_content),
        align_self: item.align_self.map(items),
        flex_direction: match flex.direction {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        flex_wrap: match flex.wrap {
            FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::FlexWrap::Wrap,
            FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        },
        flex_grow: item.grow,
        flex_shrink: item.shrink,
        flex_basis: dimension(item.basis),
        grid_template_columns: template(grid.columns),
        grid_template_rows: template(grid.rows),
        grid_auto_columns: vec![track(grid.auto_columns)],
        grid_auto_rows: vec![track(grid.auto_rows)],
        grid_auto_flow: match grid.auto_flow {
            AutoFlow::Row => taffy::GridAutoFlow::Row,
            AutoFlow::Column => taffy::GridAutoFlow::Column,
            AutoFlow::RowDense => taffy::GridAutoFlow::RowDense,
            AutoFlow::ColumnDense => taffy::GridAutoFlow::ColumnDense,
        },
        grid_column: taffy::Line {
            start: placement(cell.column.0),
            end: placement(cell.column.1),
        },
        grid_row: taffy::Line {
            start: placement(cell.row.0),
            end: placement(cell.row.1),
        },
        ..Style::default()
    }
}

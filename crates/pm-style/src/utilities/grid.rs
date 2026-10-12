//! Grid containers and grid items.

use pm_layout::{AlignItems, AutoFlow, GridLine, GridRule, Track, TrackList};
use pm_primitives::Number;

use crate::Utility;

/// `count` equal columns.
pub fn grid_cols(count: u16) -> Utility {
    GridRule::Columns(TrackList::Repeat(count, Track::Fr(1.0))).into()
}

/// `count` equal rows.
pub fn grid_rows(count: u16) -> Utility {
    GridRule::Rows(TrackList::Repeat(count, Track::Fr(1.0))).into()
}

/// `count` columns of `steps` each.
pub fn grid_cols_of(count: u16, steps: impl Number) -> Utility {
    GridRule::Columns(TrackList::Repeat(count, Track::Px(steps.step_pixels()))).into()
}

/// `count` rows of `steps` each.
pub fn grid_rows_of(count: u16, steps: impl Number) -> Utility {
    GridRule::Rows(TrackList::Repeat(count, Track::Px(steps.step_pixels()))).into()
}

/// The item spans `count` columns.
pub fn col_span(count: u16) -> Utility {
    GridRule::Column(GridLine::Span(count), GridLine::Auto).into()
}

/// The item spans `count` rows.
pub fn row_span(count: u16) -> Utility {
    GridRule::Row(GridLine::Span(count), GridLine::Auto).into()
}

/// The item starts at column line `line`; negative counts from the end.
pub fn col_start(line: i16) -> Utility {
    GridRule::ColumnStart(GridLine::Line(line)).into()
}

/// The item ends at column line `line`.
pub fn col_end(line: i16) -> Utility {
    GridRule::ColumnEnd(GridLine::Line(line)).into()
}

/// The item starts at row line `line`.
pub fn row_start(line: i16) -> Utility {
    GridRule::RowStart(GridLine::Line(line)).into()
}

/// The item ends at row line `line`.
pub fn row_end(line: i16) -> Utility {
    GridRule::RowEnd(GridLine::Line(line)).into()
}

constants! {
    /// No explicit columns.
    grid_cols_none = GridRule::Columns(TrackList::None);
    /// Columns inherited from the parent grid.
    grid_cols_subgrid = GridRule::Columns(TrackList::Subgrid);
    /// No explicit rows.
    grid_rows_none = GridRule::Rows(TrackList::None);
    /// Rows inherited from the parent grid.
    grid_rows_subgrid = GridRule::Rows(TrackList::Subgrid);
    /// The item spans every column.
    col_span_full = GridRule::Column(GridLine::Line(1), GridLine::Line(-1));
    /// The item spans every row.
    row_span_full = GridRule::Row(GridLine::Line(1), GridLine::Line(-1));
    /// The item is placed automatically along columns.
    col_auto = GridRule::Column(GridLine::Auto, GridLine::Auto);
    /// The item is placed automatically along rows.
    row_auto = GridRule::Row(GridLine::Auto, GridLine::Auto);
    /// Fill rows first.
    grid_flow_row = GridRule::AutoFlow(AutoFlow::Row);
    /// Fill columns first.
    grid_flow_col = GridRule::AutoFlow(AutoFlow::Column);
    /// Fill rows first, back-filling gaps.
    grid_flow_row_dense = GridRule::AutoFlow(AutoFlow::RowDense);
    /// Fill columns first, back-filling gaps.
    grid_flow_col_dense = GridRule::AutoFlow(AutoFlow::ColumnDense);
    /// Implicit columns sized by content.
    auto_cols_auto = GridRule::AutoColumns(Track::Auto);
    /// Implicit columns as small as their content allows.
    auto_cols_min = GridRule::AutoColumns(Track::MinContent);
    /// Implicit columns as large as their content wants.
    auto_cols_max = GridRule::AutoColumns(Track::MaxContent);
    /// Implicit columns sharing the free space.
    auto_cols_fr = GridRule::AutoColumns(Track::Fr(1.0));
    /// Implicit rows sized by content.
    auto_rows_auto = GridRule::AutoRows(Track::Auto);
    /// Implicit rows as small as their content allows.
    auto_rows_min = GridRule::AutoRows(Track::MinContent);
    /// Implicit rows as large as their content wants.
    auto_rows_max = GridRule::AutoRows(Track::MaxContent);
    /// Implicit rows sharing the free space.
    auto_rows_fr = GridRule::AutoRows(Track::Fr(1.0));
    /// Items at the start of their cell.
    justify_items_start = GridRule::JustifyItems(AlignItems::Start);
    /// Items at the end of their cell.
    justify_items_end = GridRule::JustifyItems(AlignItems::End);
    /// Items centred in their cell.
    justify_items_center = GridRule::JustifyItems(AlignItems::Center);
    /// Items stretched across their cell.
    justify_items_stretch = GridRule::JustifyItems(AlignItems::Stretch);
}

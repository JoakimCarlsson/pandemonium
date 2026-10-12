//! Grid containers and grid items.

use pm_primitives::Rule;

use crate::{AlignContent, AlignItems, JustifyContent};

/// The size of one grid track.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Track {
    /// Sized by its content and the free space.
    #[default]
    Auto,
    /// A share of the free space.
    Fr(f32),
    /// An exact number of logical pixels.
    Px(f32),
    /// As small as its content allows.
    MinContent,
    /// As large as its content wants.
    MaxContent,
    /// At least its content, at most an equal share of the free space.
    MinMax,
}

/// The tracks of one axis of a grid.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TrackList {
    /// No explicit tracks; items create implicit ones.
    #[default]
    None,
    /// `count` tracks of the same size.
    Repeat(u16, Track),
    /// The tracks are inherited from the parent grid.
    Subgrid,
}

/// How auto-placed items fill the grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AutoFlow {
    /// Fills each row before starting the next.
    #[default]
    Row,
    /// Fills each column before starting the next.
    Column,
    /// Fills rows, back-filling earlier gaps.
    RowDense,
    /// Fills columns, back-filling earlier gaps.
    ColumnDense,
}

/// One end of an item's span along a grid axis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GridLine {
    /// Placed by the auto-placement algorithm.
    #[default]
    Auto,
    /// A numbered line; negative counts from the end.
    Line(i16),
    /// Spans this many tracks.
    Span(u16),
}

/// The container half of grid layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridStyle {
    /// The column tracks.
    pub columns: TrackList,
    /// The row tracks.
    pub rows: TrackList,
    /// How auto-placed items fill the grid.
    pub auto_flow: AutoFlow,
    /// The size of implicitly created columns.
    pub auto_columns: Track,
    /// The size of implicitly created rows.
    pub auto_rows: Track,
    /// Alignment of items inside their cell along the row axis.
    pub justify_items: AlignItems,
    /// Distribution of the whole grid along the row axis.
    pub justify_content: JustifyContent,
    /// Distribution of the whole grid along the column axis.
    pub align_content: AlignContent,
}

/// The item half of grid layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridItem {
    /// Where the item starts and ends along the column axis.
    pub column: (GridLine, GridLine),
    /// Where the item starts and ends along the row axis.
    pub row: (GridLine, GridLine),
}

/// One declaration about grid layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridRule {
    /// The column tracks.
    Columns(TrackList),
    /// The row tracks.
    Rows(TrackList),
    /// Auto-placement flow.
    AutoFlow(AutoFlow),
    /// Implicit column size.
    AutoColumns(Track),
    /// Implicit row size.
    AutoRows(Track),
    /// Item alignment inside cells.
    JustifyItems(AlignItems),
    /// Grid distribution along the row axis.
    JustifyContent(JustifyContent),
    /// Grid distribution along the column axis.
    AlignContent(AlignContent),
    /// Column placement as `(start, end)`.
    Column(GridLine, GridLine),
    /// The column the item starts at.
    ColumnStart(GridLine),
    /// The column the item ends at.
    ColumnEnd(GridLine),
    /// Row placement as `(start, end)`.
    Row(GridLine, GridLine),
    /// The row the item starts at.
    RowStart(GridLine),
    /// The row the item ends at.
    RowEnd(GridLine),
}

impl Rule for GridRule {
    type Target = (GridStyle, GridItem);

    /// Writes the declaration into `target`.
    fn apply(self, (container, item): &mut (GridStyle, GridItem)) {
        match self {
            Self::Columns(tracks) => container.columns = tracks,
            Self::Rows(tracks) => container.rows = tracks,
            Self::AutoFlow(flow) => container.auto_flow = flow,
            Self::AutoColumns(track) => container.auto_columns = track,
            Self::AutoRows(track) => container.auto_rows = track,
            Self::JustifyItems(align) => container.justify_items = align,
            Self::JustifyContent(justify) => container.justify_content = justify,
            Self::AlignContent(align) => container.align_content = align,
            Self::Column(start, end) => item.column = (start, end),
            Self::ColumnStart(line) => item.column.0 = line,
            Self::ColumnEnd(line) => item.column.1 = line,
            Self::Row(start, end) => item.row = (start, end),
            Self::RowStart(line) => item.row.0 = line,
            Self::RowEnd(line) => item.row.1 = line,
        }
    }
}

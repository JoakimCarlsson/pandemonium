//! The commit graph: each commit's lanes drawn as lines and a dot, beside its name, its references and its summary.
//!
//! Each row paints its own slice of the lanes: the lines that cross its upper and lower halves and the dot of its commit. A line that changes lane runs down its own lane and across to the commit it meets, so rows stacked on each other join into one picture without any row knowing about its neighbours.

use pm_git::{Commit, Edge, Half, Lanes};
use pm_icon::IconName;
use pm_style::{
    Color, Role, absolute, bg, border_color, border_t, flex, flex_1, flex_col, flex_none,
    font_mono, gap, h, h_px, items_center, leading_ratio, left, min_h, min_w_px, overflow_hidden,
    px, relative, rounded, rounded_full, text_color, text_px, top, truncate, w_px,
};
use pm_ui::{Node, node, text};

use crate::Sash;
use crate::git_controls::{
    CHANGES_REFRESH, GRAPH_FETCH, GRAPH_PULL, GRAPH_PUSH, GRAPH_REFS, GRAPH_SASH, GRAPH_SCROLL,
    GRAPH_TARGET, GRAPH_TOGGLE,
};
use crate::parts::{glyph, line, spacer};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::{Badge, Button, badge};
use pm_input::FocusId;

/// How tall the graph is when it is open, in logical pixels.
pub const HEIGHT: f32 = 280.0;

/// The first of the rows of commits, which answer to a right press with the menu on a commit; the rest follow it.
pub const HISTORY_ROW_BASE: u64 = 8800;

/// The least and the most the graph can be made tall, in logical pixels.
pub const HEIGHTS: (f32, f32) = (120.0, 640.0);

/// How tall a row of the graph is, in logical pixels.
const ROW: f32 = 24.0;

/// How far apart two lanes sit, centre to centre, in logical pixels.
const LANE: f32 = 12.0;

/// How thick a lane's line is, in logical pixels.
const STROKE: f32 = 1.5;

/// How wide a commit's dot is, in logical pixels.
const DOT: f32 = 8.0;

/// How many branch and tag pills a commit row draws before counting the rest.
const SHOWN_REFS: usize = 2;

/// The colours branches are drawn in, in the order they are handed out.
const PALETTE: [(u8, u8, u8); 6] = [
    (0x61, 0xaf, 0xef),
    (0x98, 0xc3, 0x79),
    (0xc6, 0x78, 0xdd),
    (0xe5, 0xc0, 0x7b),
    (0x56, 0xb6, 0xc2),
    (0xe0, 0x6c, 0x75),
];

/// The colour a branch with colour index `color` is drawn in.
fn lane_color(color: usize) -> Color {
    let (red, green, blue) = PALETTE[color % PALETTE.len()];
    Color::rgb(red, green, blue)
}

/// The x coordinate of the centre of `lane` in its column.
fn lane_x(lane: usize) -> f32 {
    LANE / 2.0 + lane as f32 * LANE
}

/// A filled rectangle `width` by `height` at `x`, `y` in its cell.
fn rect(x: f32, y: f32, width: f32, height: f32, color: Color) -> Node {
    node().styles([
        absolute(),
        left(x / 4.0),
        top(y / 4.0),
        w_px(width.max(0.0)),
        h_px(height.max(0.0)),
        bg(color),
    ])
}

/// A line along `x` from `from` to `to`.
fn vertical(x: f32, from: f32, to: f32, color: Color) -> Node {
    rect(x - STROKE / 2.0, from, STROKE, to - from, color)
}

/// A line along `y` from `from` to `to`.
fn horizontal(y: f32, from: f32, to: f32, color: Color) -> Node {
    rect(
        from.min(to),
        y - STROKE / 2.0,
        (to - from).abs(),
        STROKE,
        color,
    )
}

/// The pieces that draw one line through half of the row.
fn edge(edge: &Edge) -> Vec<Node> {
    let color = lane_color(edge.color);
    let (from, to) = (lane_x(edge.from), lane_x(edge.to));
    let middle = ROW / 2.0;
    let (top, bottom) = match edge.half {
        Half::Upper => (0.0, middle),
        Half::Lower => (middle, ROW),
    };
    if edge.from == edge.to {
        return vec![vertical(from, top, bottom, color)];
    }
    match edge.half {
        Half::Upper => vec![
            vertical(from, top, middle, color),
            horizontal(middle, from, to, color),
        ],
        Half::Lower => vec![
            horizontal(middle, from, to, color),
            vertical(to, middle, bottom, color),
        ],
    }
}

/// One commit's slice of the graph, in a column `columns` lanes wide.
fn cell(lanes: &Lanes, columns: usize) -> Node {
    let dot = rect(
        lane_x(lanes.lane) - DOT / 2.0,
        ROW / 2.0 - DOT / 2.0,
        DOT,
        DOT,
        lane_color(lanes.color),
    )
    .styles([rounded_full()]);
    node()
        .styles([
            relative(),
            flex_none(),
            w_px(columns.max(1) as f32 * LANE),
            h_px(ROW),
        ])
        .children(lanes.edges.iter().flat_map(edge))
        .child(dot)
}

/// The pill naming a branch or tag that points at a commit.
///
/// Git decorates the checked out branch as `HEAD -> name` and a tag as `tag: name`; the pill says the name, and the branch HEAD is on is drawn solid.
fn reference_badge(decorated: &str, color: Color) -> Node {
    let (name, current) = match decorated.strip_prefix("HEAD -> ") {
        Some(branch) => (branch, true),
        None => (decorated.strip_prefix("tag: ").unwrap_or(decorated), false),
    };
    Badge::new(name)
        .tone(color)
        .variant(if current {
            badge::Variant::Solid
        } else {
            badge::Variant::Soft
        })
        .square()
        .build()
}

/// One commit of the graph: its lanes, its name, its references and its summary.
fn commit_row(commit: &Commit, columns: usize, position: usize) -> Node {
    let color = lane_color(commit.lanes.color);
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h_px(ROW),
            px(3),
            flex_none(),
            overflow_hidden(),
        ])
        .child(cell(&commit.lanes, columns))
        .child(text(commit.id.clone()).styles([
            text_px(11),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Muted),
            flex_none(),
        ]))
        .children(
            commit
                .refs
                .iter()
                .take(SHOWN_REFS)
                .map(|name| reference_badge(name, color)),
        );
    if commit.refs.len() > SHOWN_REFS {
        row = row.child(
            text(format!("+{}", commit.refs.len() - SHOWN_REFS)).styles([
                text_px(11),
                leading_ratio(1.4),
                text_color(Role::Subtle),
                flex_none(),
            ]),
        );
    }
    row.child(
        node()
            .styles([flex_1(), min_w_px(0), overflow_hidden()])
            .child(line(&commit.summary, 12, 1.4, Role::Text).styles([truncate()])),
    )
    .styles([bg(Role::SurfaceHover).hover()])
    .focusable(FocusId(HISTORY_ROW_BASE + position as u64))
}

/// A box as tall as `rows` rows, standing in for those left out.
fn blank(rows: usize) -> Node {
    node().styles([h_px(rows as f32 * ROW), flex_none()])
}

/// The rows of `commits` that are in a body `height` tall scrolled `scroll` down, with a blank for those above and below.
fn visible(commits: &[Commit], columns: usize, scroll: f32, height: f32) -> Vec<Node> {
    let first = ((scroll / ROW).floor().max(0.0) as usize).min(commits.len());
    let last = (((scroll + height) / ROW).ceil() as usize + 1).min(commits.len());
    let mut rows = vec![blank(first)];
    rows.extend(
        commits[first..last]
            .iter()
            .enumerate()
            .map(|(offset, commit)| commit_row(commit, columns, first + offset)),
    );
    rows.push(blank(commits.len() - last));
    rows
}

/// The button saying which references the graph lists, which switches between the checked out branch and all of them.
fn refs_button(all: bool) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            gap(1),
            h(6),
            px(2),
            rounded(pm_style::Radius::Md),
            pm_style::cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(glyph(IconName::GitFork, 3.5, Role::Muted))
        .child(line(if all { "All" } else { "Auto" }, 12, 1.4, Role::Text))
        .child(glyph(IconName::ChevronDown, 3.0, Role::Muted))
        .focusable(GRAPH_REFS)
}

/// The graph of the open session's repository, pinned under the changes: a strip of what can be done to the remote, then the commits, with a handle above it that drags to change how tall it is.
pub fn graph(state: &WorkspaceState) -> Node {
    let git = &state.git;
    let open = git.graph_open;
    let height = git.graph_height.clamp(HEIGHTS.0, HEIGHTS.1);
    let strip = node()
        .styles([flex(), items_center(), h(7), px(2), flex_none(), gap(0.5)])
        .child(
            Button::icon(
                if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                },
                GRAPH_TOGGLE,
            )
            .size(Size::Sm)
            .build()
            .tooltip(if open {
                "Collapse Graph"
            } else {
                "Expand Graph"
            }),
        )
        .child(line("Graph", 13, 1.4, Role::Text))
        .child(spacer())
        .child(refs_button(git.graph_all).tooltip("Choose history references"))
        .child(
            Button::icon(IconName::Target, GRAPH_TARGET)
                .size(Size::Sm)
                .build()
                .tooltip("Go to current history item"),
        )
        .child(
            Button::icon(IconName::GitFetch, GRAPH_FETCH)
                .size(Size::Sm)
                .build()
                .tooltip("Fetch from all remotes"),
        )
        .child(
            Button::icon(IconName::GitPull, GRAPH_PULL)
                .size(Size::Sm)
                .build()
                .tooltip("Pull"),
        )
        .child(
            Button::icon(IconName::GitPush, GRAPH_PUSH)
                .size(Size::Sm)
                .build()
                .tooltip("Push"),
        )
        .child(
            Button::icon(IconName::Refresh, CHANGES_REFRESH)
                .size(Size::Sm)
                .spinning(git.working)
                .build()
                .tooltip("Refresh"),
        );
    let mut panel = node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            h_px(if open { height } else { 28.0 }),
            border_t(),
            border_color(Role::Border),
            overflow_hidden(),
        ])
        .child(strip);
    if open {
        let columns = git
            .history
            .iter()
            .map(|commit| commit.lanes.width)
            .max()
            .unwrap_or(1);
        let rows = if git.history.is_empty() {
            vec![node().styles([px(4), pm_style::py(2)]).child(line(
                "No commits.",
                12,
                1.4,
                Role::Subtle,
            ))]
        } else {
            visible(&git.history, columns, git.graph_scroll, height - 28.0)
        };
        panel = panel.child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(GRAPH_SCROLL, git.graph_scroll)
                .child(
                    node()
                        .styles([flex(), flex_col(), flex_none()])
                        .children(rows),
                ),
        );
    }
    let mut column = node().styles([flex(), flex_col(), flex_none()]);
    if open {
        column = column.child(Sash::row(GRAPH_SASH).build());
    }
    column.child(panel)
}

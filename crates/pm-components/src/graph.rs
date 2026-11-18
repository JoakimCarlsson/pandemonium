//! The commit graph: each commit's lanes drawn as lines and a dot, beside its name, its references and its summary.
//!
//! Each row paints its own slice of the lanes: the lines that cross its upper and lower halves and the dot of its commit. A line that changes lane runs down its own lane and across to the commit it meets, so rows stacked on each other join into one picture without any row knowing about its neighbours.

use crate::kit::sp;
use pm_git::{Commit, Edge, Half, Lanes};
use rustle::{CursorIcon, Modifier};
use rustle::{Flex, flex_column, flex_row};

use crate::Sash;
use crate::git_controls::{
    CHANGES_REFRESH, GRAPH_FETCH, GRAPH_PULL, GRAPH_PUSH, GRAPH_REFS, GRAPH_SASH, GRAPH_SCROLL,
    GRAPH_TARGET, GRAPH_TOGGLE,
};
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Scroll, Side, Styled, Tone,
    Tooltip, Type, label, label_with, scroll_area,
};
use crate::parts::{glyph, line, spacer};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::{Badge, Button, badge};

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
fn lane_color(color: usize) -> Tone {
    let (red, green, blue) = PALETTE[color % PALETTE.len()];
    Tone::rgb(red, green, blue)
}

/// The x coordinate of the centre of `lane` in its column.
fn lane_x(lane: usize) -> f32 {
    LANE / 2.0 + lane as f32 * LANE
}

/// Emits a filled rectangle `width` by `height` at `x`, `y` in its cell, with corners rounded by `radius`.
#[track_caller]
fn rect(x: f32, y: f32, width: f32, height: f32, color: Tone, radius: Radius) {
    rustle::spacer(
        Modifier::new()
            .at(x, y)
            .width(width.max(0.0))
            .height(height.max(0.0))
            .fill_rounded(color, radius),
    );
}

/// Emits a line along `x` from `from` to `to`.
#[track_caller]
fn vertical(x: f32, from: f32, to: f32, color: Tone) {
    rect(
        x - STROKE / 2.0,
        from,
        STROKE,
        to - from,
        color,
        Radius::None,
    );
}

/// Emits a line along `y` from `from` to `to`.
#[track_caller]
fn horizontal(y: f32, from: f32, to: f32, color: Tone) {
    rect(
        from.min(to),
        y - STROKE / 2.0,
        (to - from).abs(),
        STROKE,
        color,
        Radius::None,
    );
}

/// Emits the pieces that draw one line through half of the row.
#[track_caller]
fn edge(edge: &Edge) {
    let color = lane_color(edge.color);
    let (from, to) = (lane_x(edge.from), lane_x(edge.to));
    let middle = ROW / 2.0;
    let (top, bottom) = match edge.half {
        Half::Upper => (0.0, middle),
        Half::Lower => (middle, ROW),
    };
    if edge.from == edge.to {
        vertical(from, top, bottom, color);
        return;
    }
    match edge.half {
        Half::Upper => {
            vertical(from, top, middle, color);
            horizontal(middle, from, to, color);
        }
        Half::Lower => {
            horizontal(middle, from, to, color);
            vertical(to, middle, bottom, color);
        }
    }
}

/// Emits one commit's slice of the graph, in a column `columns` lanes wide.
#[track_caller]
fn cell(lanes: &Lanes, columns: usize) {
    rustle::stack_with(
        Modifier::new()
            .width(columns.max(1) as f32 * LANE)
            .height(ROW),
        || {
            for piece in &lanes.edges {
                edge(piece);
            }
            rect(
                lane_x(lanes.lane) - DOT / 2.0,
                ROW / 2.0 - DOT / 2.0,
                DOT,
                DOT,
                lane_color(lanes.color),
                Radius::Full,
            );
        },
    );
}

/// Emits the pill naming a branch or tag that points at a commit.
///
/// Git decorates the checked out branch as `HEAD -> name` and a tag as `tag: name`; the pill says the name, and the branch HEAD is on is drawn solid.
#[track_caller]
fn reference_badge(decorated: &str, color: Tone) {
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
        .build();
}

/// Emits one commit of the graph, the `position`-th of the list: its lanes, its name, its references and its summary.
#[track_caller]
fn commit_row(commit: &Commit, columns: usize, position: usize) {
    let color = lane_color(commit.lanes.color);
    let id = FocusId(HISTORY_ROW_BASE + position as u64);
    flex_row(
        Modifier::new()
            .height(ROW)
            .clip_to(Radius::None)
            .looks_of(Looks::new().hover(Role::SurfaceHover), id)
            .target(id)
            .pad_x(3),
        Flex::new().items_center().gap(sp(2)),
        || {
            cell(&commit.lanes, columns);
            label(
                commit.id.clone(),
                Type::new(11).ratio(1.4).mono().tone(Role::Muted),
            );
            for name in commit.refs.iter().take(SHOWN_REFS) {
                reference_badge(name, color);
            }
            if commit.refs.len() > SHOWN_REFS {
                label(
                    format!("+{}", commit.refs.len() - SHOWN_REFS),
                    Type::new(11).ratio(1.4).tone(Role::Subtle),
                );
            }
            label_with(
                Modifier::new().grow(),
                &commit.summary,
                Type::new(12).ratio(1.4).tone(Role::Text).truncate(),
            );
        },
    );
}

/// Emits a box as tall as `rows` rows, standing in for those left out.
#[track_caller]
fn blank(rows: usize) {
    rustle::spacer(Modifier::new().height(rows as f32 * ROW));
}

/// Emits the rows of `commits` that are in a body `height` tall scrolled `scroll` down, with a blank for those above and below.
#[track_caller]
fn visible(commits: &[Commit], columns: usize, scroll: f32, height: f32) {
    let first = ((scroll / ROW).floor().max(0.0) as usize).min(commits.len());
    let last = (((scroll + height) / ROW).ceil() as usize + 1).min(commits.len());
    blank(first);
    for (offset, commit) in commits[first..last].iter().enumerate() {
        rustle::key(first + offset, || {
            commit_row(commit, columns, first + offset)
        });
    }
    blank(commits.len() - last);
}

/// Emits the button saying which references the graph lists, which switches between the checked out branch and all of them, with `tip` shown while it is hovered.
#[track_caller]
fn refs_button(all: bool, tip: &str) {
    flex_row(
        Modifier::new()
            .h(6)
            .tooltip(tip)
            .looks_of(
                Looks::new().hover(Role::SurfaceHover).radius(Radius::Md),
                GRAPH_REFS,
            )
            .cursor(CursorIcon::Pointer)
            .target(GRAPH_REFS)
            .pad_x(2),
        Flex::new().items_center().gap(sp(1)),
        || {
            glyph(IconName::GitFork, 3.5, Role::Muted);
            line(if all { "All" } else { "Auto" }, 12, 1.4, Role::Text);
            glyph(IconName::ChevronDown, 3.0, Role::Muted);
        },
    );
}

/// Emits a small button showing `icon` that answers to `id`, spinning while `spinning`, with `tip` shown while it is hovered, giving up room with the strip's other items when the strip is too narrow, as a CSS flex item does.
#[track_caller]
fn strip_button(icon: IconName, id: FocusId, spinning: bool, tip: &str) {
    Button::icon(icon, id)
        .size(Size::Sm)
        .spinning(spinning)
        .modifier(Modifier::new().tooltip(tip).shrink(1.0))
        .build();
}

/// Emits the strip over the graph: the button that folds it, its name, and what can be done to the remote.
#[track_caller]
fn strip(state: &WorkspaceState) {
    let git = &state.git;
    let open = git.graph_open;
    flex_row(
        Modifier::new().h(7).pad_x(2),
        Flex::new().items_center().gap(sp(0.5)),
        || {
            strip_button(
                if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                },
                GRAPH_TOGGLE,
                false,
                if open {
                    "Collapse Graph"
                } else {
                    "Expand Graph"
                },
            );
            line("Graph", 13, 1.4, Role::Text);
            spacer();
            refs_button(git.graph_all, "Choose history references");
            strip_button(
                IconName::Target,
                GRAPH_TARGET,
                false,
                "Go to current history item",
            );
            strip_button(
                IconName::GitFetch,
                GRAPH_FETCH,
                false,
                "Fetch from all remotes",
            );
            strip_button(IconName::GitPull, GRAPH_PULL, false, "Pull");
            strip_button(IconName::GitPush, GRAPH_PUSH, false, "Push");
            strip_button(IconName::Refresh, CHANGES_REFRESH, git.working, "Refresh");
        },
    );
}

/// Emits the graph of the open session's repository, pinned under the changes: a strip of what can be done to the remote, then the commits, with a handle above it that drags to change how tall it is.
#[track_caller]
pub fn graph(state: &WorkspaceState) {
    let git = &state.git;
    let open = git.graph_open;
    let height = git.graph_height.clamp(HEIGHTS.0, HEIGHTS.1);
    flex_column(Modifier::new(), Flex::new(), || {
        if open {
            Sash::row(GRAPH_SASH).build();
        }
        flex_column(
            Modifier::new()
                .height(if open { height } else { 28.0 })
                .stroke_side(Side::Top, 1.0, Role::Border)
                .clip_to(Radius::None),
            Flex::new(),
            || {
                strip(state);
                if !open {
                    return;
                }
                let columns = git
                    .history
                    .iter()
                    .map(|commit| commit.lanes.width)
                    .max()
                    .unwrap_or(1);
                scroll_area(
                    Modifier::new().grow(),
                    Scroll::from_start(GRAPH_SCROLL, git.graph_scroll),
                    || {
                        flex_column(Modifier::new().fill_max_width(), Flex::new(), || {
                            if git.history.is_empty() {
                                flex_column(Modifier::new().pad_x(4).pad_y(2), Flex::new(), || {
                                    line("No commits.", 12, 1.4, Role::Subtle);
                                });
                            } else {
                                visible(&git.history, columns, git.graph_scroll, height - 28.0);
                            }
                        });
                    },
                );
            },
        );
    });
}

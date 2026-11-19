//! The commit graph: each commit's lanes drawn as lines and a dot, beside its name, its references and its summary.
//!
//! Each row paints its own slice of the lanes: the lines that cross its upper and lower halves and the dot of its commit. A line that changes lane runs down its own lane and across to the commit it meets, so rows stacked on each other join into one picture without any row knowing about its neighbours.

use crate::events::reduce;
use crate::git_menus::MenuKind;
use crate::theme;
use crate::theme::sp;
use pm_git::{Commit, Edge, Half, Lanes};
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};
use rustle::{Icon, phosphor};

use crate::Sash;
use crate::events::Interactive;
use crate::focus::FocusId;
use crate::git_controls::{
    CHANGES_REFRESH, GRAPH_FETCH, GRAPH_PULL, GRAPH_PUSH, GRAPH_REFS, GRAPH_SASH, GRAPH_TARGET,
    GRAPH_TOGGLE,
};
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::theme::{Radius, Role, Tone};
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
            .offset(x, y)
            .width(width.max(0.0))
            .height(height.max(0.0))
            .background_rounded(theme::color(color), radius.px()),
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
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(ROW)
            .clip()
            .interaction_focused(crate::focus::is_held(id))
            .interaction_background(
                (theme::InteractionTones::new().hover(Role::SurfaceHover)).colors(),
            )
            .control_menu(
                id,
                {
                    let pressed = reduce(id);
                    move |_| pressed()
                },
                {
                    let operations = crate::events::operations();
                    move || operations.open_git_menu(MenuKind::History(position), None)
                },
            )
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            cell(&commit.lanes, columns);
            rustle::basic_text(
                Modifier::new(),
                commit.id.clone(),
                theme::mono_style(11, 1.4).color(theme::color(Role::Muted)),
                rustle::TextOverflow::Ellipsis,
            );
            for name in commit.refs.iter().take(SHOWN_REFS) {
                reference_badge(name, color);
            }
            if commit.refs.len() > SHOWN_REFS {
                rustle::basic_text(
                    Modifier::new(),
                    format!("+{}", commit.refs.len() - SHOWN_REFS),
                    theme::text_style(11, 1.4).color(theme::color(Role::Subtle)),
                    rustle::TextOverflow::Ellipsis,
                );
            }
            rustle::basic_text(
                Modifier::new().weight(1.0),
                &commit.summary,
                theme::text_style(12, 1.4)
                    .color(theme::color(Role::Text))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
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
    crate::tooltip::with_tooltip(tip, || {
        row_arranged(
            Modifier::new()
                .height(theme::sp(6))
                .interaction_focused(crate::focus::is_held(GRAPH_REFS))
                .interaction_background(
                    (theme::InteractionTones::new()
                        .hover(Role::SurfaceHover)
                        .radius(Radius::Md))
                    .colors(),
                )
                .cursor(CursorIcon::Pointer)
                .control(GRAPH_REFS, {
                    let operations = crate::events::operations();
                    move || operations.open_git_menu(MenuKind::Refs, Some(GRAPH_REFS))
                })
                .padding_xy(theme::sp(2), 0.0),
            Arrangement::SpacedBy(sp(1)),
            Alignment::Center,
            || {
                glyph(phosphor::GIT_FORK, 3.5, Role::Muted);
                line(if all { "All" } else { "Auto" }, 12, 1.4, Role::Text);
                glyph(phosphor::CARET_DOWN, 3.0, Role::Muted);
            },
        );
    });
}

/// Emits a small button showing `icon` that answers to `id`, spinning while `spinning`, with `tip` shown while it is hovered.
#[track_caller]
fn strip_button(icon: Icon, id: FocusId, spinning: bool, tip: &str) {
    Button::icon(icon, id)
        .size(Size::Sm)
        .spinning(spinning)
        .tooltip(tip)
        .build();
}

/// Emits the strip over the graph: the button that folds it, its name, and what can be done to the remote.
#[track_caller]
fn strip(state: &WorkspaceState) {
    let git = &state.git;
    let open = git.graph_open;
    rustle::layout(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(7))
            .padding_xy(theme::sp(2), 0.0),
        StripLayout { gap: sp(0.5) },
        || {
            strip_button(
                if open {
                    phosphor::CARET_DOWN
                } else {
                    phosphor::CARET_RIGHT
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
            refs_button(git.graph_all, "Choose history references");
            strip_button(
                phosphor::TARGET,
                GRAPH_TARGET,
                false,
                "Go to current history item",
            );
            strip_button(
                phosphor::CLOUD_ARROW_DOWN,
                GRAPH_FETCH,
                false,
                "Fetch from all remotes",
            );
            strip_button(phosphor::ARROW_LINE_DOWN, GRAPH_PULL, false, "Pull");
            strip_button(phosphor::ARROW_LINE_UP, GRAPH_PUSH, false, "Push");
            strip_button(
                phosphor::ARROWS_CLOCKWISE,
                CHANGES_REFRESH,
                git.working,
                "Refresh",
            );
        },
    );
}

/// How the strip over the graph lays out its children: the fold button and the name at the
/// start, the references button and the remote buttons after it at the end, `gap` apart and at
/// least two gaps between the two groups.
///
/// When they do not fit, every child but the references button gives up the overflow in
/// proportion to its width, the name cutting itself short and the buttons drawing narrower.
struct StripLayout {
    /// The room between neighbours, in logical pixels.
    gap: f32,
}

impl StripLayout {
    /// Where the references button is among the strip's children: the first of the group at the end.
    const REFS: usize = 2;
}

impl rustle::Layout for StripLayout {
    /// Measures every child at its own width or, when the strip is too narrow, its share, and
    /// centres each down the strip.
    fn measure(
        &self,
        children: &mut rustle::Children,
        constraints: rustle::Constraints,
    ) -> rustle::LayoutResult {
        let height = constraints.max_height;
        let room = constraints.max_width;
        let count = children.len();
        let natural: Vec<f32> = (0..count)
            .map(|index| children.max_intrinsic_width(index, height))
            .collect();
        let gaps = self.gap * count as f32;
        let fixed = natural.get(Self::REFS).copied().unwrap_or(0.0) + gaps;
        let giving: f32 = natural.iter().sum::<f32>() + gaps - fixed;
        let scale = if giving > 0.0 {
            ((room - fixed).max(0.0) / giving).min(1.0)
        } else {
            1.0
        };
        let widths: Vec<f32> = natural
            .iter()
            .enumerate()
            .map(|(index, width)| {
                if index == Self::REFS {
                    *width
                } else {
                    width * scale
                }
            })
            .collect();
        let trailing: f32 = widths.iter().skip(Self::REFS).sum::<f32>()
            + self.gap * (count - Self::REFS.min(count)).saturating_sub(1) as f32;
        let mut positions = Vec::with_capacity(count);
        let mut along = 0.0;
        for (index, width) in widths.iter().enumerate() {
            if index == Self::REFS {
                along = room - trailing;
            }
            let size = children.measure(
                index,
                rustle::Constraints {
                    min_width: *width,
                    max_width: *width,
                    min_height: 0.0,
                    max_height: height,
                },
            );
            positions.push(rustle::Offset::new(along, (height - size.height) / 2.0));
            along += width + self.gap;
        }
        rustle::LayoutResult {
            size: rustle::Size::new(room, height),
            positions,
        }
    }
}

/// Emits the graph of the open session's repository, pinned under the changes: a strip of what can be done to the remote, then the commits, with a handle above it that drags to change how tall it is.
#[track_caller]
pub fn graph(state: &WorkspaceState) {
    let git = &state.git;
    let open = git.graph_open;
    let height = git.graph_height.clamp(HEIGHTS.0, HEIGHTS.1);
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            if open {
                Sash::row(GRAPH_SASH)
                    .modifier(Modifier::new().fill_max_width())
                    .build();
            }
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(if open { height } else { 28.0 })
                    .draw_behind(theme::edge(
                        theme::Side::Top,
                        1.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(0.0, 1.0, 0.0, 0.0)
                    .clip(),
                Arrangement::Start,
                Alignment::Start,
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
                    column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .weight(1.0)
                            .vertical_scroll(&git.graph_scroll)
                            .fill_max_width(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            if git.history.is_empty() {
                                column_arranged(
                                    Modifier::new()
                                        .padding_xy(theme::sp(4), 0.0)
                                        .padding_xy(0.0, theme::sp(2)),
                                    Arrangement::Start,
                                    Alignment::Start,
                                    || {
                                        line("No commits.", 12, 1.4, Role::Subtle);
                                    },
                                );
                            } else {
                                visible(
                                    &git.history,
                                    columns,
                                    git.graph_scroll.offset(),
                                    height - 28.0,
                                );
                            }
                        },
                    );
                },
            );
        },
    );
}

//! The commit graph: each commit's lanes drawn as lines and a dot, beside its name, its references and its summary.
//!
//! Each row paints its own slice of the lanes: the lines that cross its upper and lower halves and the dot of its commit. A line that changes lane runs down its own lane and across to the commit it meets, so rows stacked on each other join into one picture without any row knowing about its neighbours.

use crate::areas::GitEvent;
use crate::events::{Event, send};
use crate::git_menus::MenuKind;
use crate::icons;
use crate::parts;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::sp;
use pm_git::{Commit, Edge, Half, Lanes};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, TextOverflow};
use rustle::{Color, CursorIcon, Modifier};

use crate::Sash;
use crate::events::Interactive;

use crate::parts::{glyph, line};
use crate::review::small_icon_button;
use crate::state::WorkspaceState;
use crate::theme::{Radius, Role};
use crate::{Badge, badge};

/// How tall the graph is when it is open, in logical pixels.
pub const HEIGHT: f32 = 280.0;

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

/// Emits a filled rectangle `width` by `height` at `x`, `y` in its cell, with corners rounded by `radius`.
#[track_caller]
fn rect(x: f32, y: f32, width: f32, height: f32, color: Color, radius: Radius) {
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
fn vertical(x: f32, from: f32, to: f32, color: Color) {
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
fn horizontal(y: f32, from: f32, to: f32, color: Color) {
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
fn reference_badge(decorated: &str, color: Color) {
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
fn commit_row(commit: &Commit, columns: usize, position: usize) {
    let color = lane_color(commit.lanes.color);
    let id = Target::Git(GitEvent::HistoryRow(position));
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(ROW)
            .clip()
            .indicated(
                theme::InteractionTones::new().hover(Role::SurfaceHover),
                theme::Focused::Control(crate::focus::is_focused(id)),
            )
            .control_menu(
                id,
                {
                    let pressed = send(Event::pressing(id));
                    move |_| pressed()
                },
                {
                    let operations = crate::events::operations();
                    move |at| operations.open_git_menu(MenuKind::History(position), Some(at))
                },
            )
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            cell(&commit.lanes, columns);
            rustle::basic_text(
                Modifier::new(),
                commit.id.clone(),
                theme::mono_style(11, 1.4).color(theme::color(Role::Muted)),
                TextOverflow::Ellipsis,
            );
            for name in commit.refs.iter().take(SHOWN_REFS) {
                reference_badge(name, color);
            }
            if commit.refs.len() > SHOWN_REFS {
                rustle::basic_text(
                    Modifier::new(),
                    format!("+{}", commit.refs.len() - SHOWN_REFS),
                    theme::text_style(11, 1.4).color(theme::color(Role::Subtle)),
                    TextOverflow::Ellipsis,
                );
            }
            rustle::basic_text(
                Modifier::new().weight(1.0),
                &commit.summary,
                theme::line(12, Role::Text),
                TextOverflow::Ellipsis,
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
fn refs_button(state: &WorkspaceState, tip: &str) {
    let all = state.git.graph_all;
    let menu = state.git_menu_under(MenuKind::Refs);
    crate::tooltip::with_tooltip(tip, || {
        rustle::row_arranged(
            Modifier::new()
                .height(theme::sp(6))
                .indicated(
                    theme::InteractionTones::new()
                        .hover(Role::SurfaceHover)
                        .radius(Radius::Md),
                    theme::Focused::Control(crate::focus::is_focused(Target::Git(
                        GitEvent::GraphRefs,
                    ))),
                )
                .cursor(CursorIcon::Pointer)
                .control(Target::Git(GitEvent::GraphRefs), {
                    let operations = crate::events::operations();
                    move || operations.open_git_menu(MenuKind::Refs, None)
                })
                .padding_xy(theme::sp(2), 0.0),
            Arrangement::SpacedBy(theme::sp(1)),
            Alignment::Center,
            || {
                glyph(phosphor::GIT_FORK, 3.5, Role::Muted);
                line(if all { "All" } else { "Auto" }, 12, 1.4, Role::Text);
                glyph(phosphor::CARET_DOWN, 3.0, Role::Muted);
                if let Some(menu) = menu {
                    menu.build();
                }
            },
        );
    });
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
        SplitLayout {
            gap: sp(0.5),
            split: 2,
            fixed: &[2],
        },
        || {
            let fold = if open {
                "Collapse Graph"
            } else {
                "Expand Graph"
            };
            small_icon_button(
                icons::chevron(open),
                Target::Git(GitEvent::GraphToggle),
                false,
                fold,
            );
            line("Graph", 13, 1.4, Role::Text);
            refs_button(state, "Choose history references");
            for (icon, id, spinning, tip) in [
                (
                    phosphor::TARGET,
                    GitEvent::GraphTarget,
                    false,
                    "Go to current history item",
                ),
                (
                    phosphor::CLOUD_ARROW_DOWN,
                    GitEvent::GraphFetch,
                    false,
                    "Fetch from all remotes",
                ),
                (
                    phosphor::ARROW_LINE_DOWN,
                    GitEvent::GraphPull,
                    false,
                    "Pull",
                ),
                (phosphor::ARROW_LINE_UP, GitEvent::GraphPush, false, "Push"),
                (
                    phosphor::ARROWS_CLOCKWISE,
                    GitEvent::ChangesRefresh,
                    git.working,
                    "Refresh",
                ),
            ] {
                small_icon_button(icon, Target::Git(id), spinning, tip);
            }
        },
    );
}

/// How a strip lays out its children in two groups `gap` apart: those before `split` from its
/// start and the rest against its end, at least two gaps between the groups.
///
/// When they do not fit, every child but those at `fixed` gives up the overflow in proportion to
/// its width, a text cutting itself short and a button drawing narrower.
pub(crate) struct SplitLayout {
    /// The room between neighbours, in logical pixels.
    pub gap: f32,
    /// Where the group at the end begins among the children.
    pub split: usize,
    /// The children that keep their own widths.
    pub fixed: &'static [usize],
}

impl rustle::Layout for SplitLayout {
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
        let kept = |index: &usize| self.fixed.contains(index);
        let fixed = (0..count)
            .filter(kept)
            .map(|index| natural[index])
            .sum::<f32>()
            + gaps;
        let giving: f32 = natural.iter().sum::<f32>() + gaps - fixed;
        let scale = if giving > 0.0 {
            ((room - fixed).max(0.0) / giving).min(1.0)
        } else {
            1.0
        };
        let widths: Vec<f32> = natural
            .iter()
            .enumerate()
            .map(|(index, width)| if kept(&index) { *width } else { width * scale })
            .collect();
        let trailing: f32 = widths.iter().skip(self.split).sum::<f32>()
            + self.gap * (count - self.split.min(count)).saturating_sub(1) as f32;
        let mut positions = Vec::with_capacity(count);
        let mut along = 0.0;
        for (index, width) in widths.iter().enumerate() {
            if index == self.split {
                along = room - trailing;
            }
            let size = children.measure(
                index,
                rustle::Constraints {
                    min_width: *width,
                    ..rustle::Constraints::loose(rustle::Size::new(*width, height))
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
    let height = git.graph_height.get().clamp(HEIGHTS.0, HEIGHTS.1);
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            if open {
                Sash::row(Target::Git(GitEvent::GraphSash))
                    .modifier(Modifier::new().fill_max_width())
                    .build();
            }
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(if open { height } else { 28.0 }),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
                    rustle::column_arranged(
                        Modifier::new().fill_max_width().weight(1.0).clip(),
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
                            rustle::column_arranged(
                                Modifier::new()
                                    .fill_max_width()
                                    .weight(1.0)
                                    .vertical_scroll(&git.graph_scroll)
                                    .fill_max_width(),
                                Arrangement::Start,
                                Alignment::Start,
                                || {
                                    if git.history.is_empty() {
                                        parts::note(4, 2, "No commits.", 12, Role::Subtle);
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
        },
    );
}

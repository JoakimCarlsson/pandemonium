//! What floats over the screen: popovers placed against what opened them,
//! menus opened at a point of the window, and tooltips, all kept inside the
//! window and drawn over everything, unclipped by the boxes they are declared in.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle::{Children, Constraints, Layout, LayoutResult, Modifier, Offset, Rect, Size};

use crate::kit::frame;
use crate::kit::layout::{Flex, Styled, column};
use crate::kit::report;
use crate::kit::text::{Type, label};
use crate::kit::theme::{Radius, Role};

/// How far a tooltip stays from the box it is about, in logical pixels.
const TOOLTIP_GAP: f32 = 4.0;

/// The layer menus and popovers are drawn in, over everything else.
pub const POPOVER_LAYER: i32 = 40;

/// The layer menus opened at a point are drawn in.
pub const MENU_LAYER: i32 = 50;

/// The layer tooltips are drawn in, over menus.
const TOOLTIP_LAYER: i32 = 60;

/// Where a popover opens against what opened it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    /// Above it, their left edges lined up: `absolute()`, `inset_length(Bottom, 100%)`, `left(0)`.
    Above,
    /// Above it, their right edges lined up: the same with `right(0)`.
    AboveEnd,
    /// Below it, their left edges lined up.
    Below,
    /// Below it, their right edges lined up.
    BelowEnd,
}

/// Lays the trigger out as it likes and the popover beside it, unbounded, without taking room.
struct Anchored {
    /// Where the popover goes.
    placement: Placement,
    /// The space between the trigger and the popover, in logical pixels.
    gap: f32,
}

impl Layout for Anchored {
    fn measure(&self, children: &mut Children, constraints: Constraints) -> LayoutResult {
        let trigger = if children.is_empty() {
            Size::new(0.0, 0.0)
        } else {
            children.measure(0, constraints)
        };
        let mut positions = vec![Offset::new(0.0, 0.0)];
        if children.len() > 1 {
            let open = Constraints {
                min_width: 0.0,
                max_width: f32::INFINITY,
                min_height: 0.0,
                max_height: f32::INFINITY,
            };
            let popover = children.measure(1, open);
            let x = match self.placement {
                Placement::Above | Placement::Below => 0.0,
                Placement::AboveEnd | Placement::BelowEnd => trigger.width - popover.width,
            };
            let y = match self.placement {
                Placement::Above | Placement::AboveEnd => -popover.height - self.gap,
                Placement::Below | Placement::BelowEnd => trigger.height + self.gap,
            };
            positions.push(Offset::new(x, y));
            for index in 2..children.len() {
                children.measure(index, open);
                positions.push(Offset::new(x, y));
            }
        }
        positions.truncate(children.len());
        LayoutResult {
            size: trigger,
            positions,
        }
    }
}

/// Emits `trigger`, and while `popover` is given, the popover placed against
/// it as `placement` says, `gap` steps away, over everything and kept inside
/// the window: `anchored(button, popover)` with `Popover::over_button`.
#[track_caller]
pub fn anchored(
    modifier: Modifier,
    placement: Placement,
    gap: f32,
    trigger: impl FnOnce(),
    popover: Option<impl FnOnce()>,
) {
    let gap = crate::kit::theme::sp(gap);
    rustle::layout(modifier, Anchored { placement, gap }, move || {
        trigger();
        if let Some(popover) = popover {
            rustle::stack_with(
                Modifier::new()
                    .z_index_unclipped(POPOVER_LAYER)
                    .keep_in_window(),
                popover,
            );
        }
    });
}

/// Measures its children without bounds and takes no room itself, so what
/// it holds floats where it is placed without moving its siblings.
struct Detached;

impl Layout for Detached {
    fn measure(&self, children: &mut Children, _: Constraints) -> LayoutResult {
        let open = Constraints {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        };
        for index in 0..children.len() {
            children.measure(index, open);
        }
        LayoutResult {
            size: Size::new(0.0, 0.0),
            positions: vec![Offset::new(0.0, 0.0); children.len()],
        }
    }
}

/// Emits `content` in `modifier` taking no room where it is declared, in the
/// layer `z`, with its top left at (`x`, `y`) in the window, kept inside the
/// window's frame, clear of the band around it that holds its shadow, where a
/// press resizes the window instead of reaching it.
#[track_caller]
fn detached_at(x: f32, y: f32, z: i32, modifier: Modifier, content: impl FnOnce()) {
    let band = crate::shadow_margin();
    rustle::layout(Modifier::new(), Detached, move || {
        rustle::stack_with(
            Modifier::new()
                .z_index_unclipped(z)
                .in_window(x - band, y - band)
                .keep_in_window()
                .padding(band),
            || rustle::stack_with(modifier, content),
        );
    });
}

/// Emits `content` with its top left at (`x`, `y`) in the window, over
/// everything and kept inside the window, taking no room where it is
/// declared: a menu opened where the pointer was, `absolute()`,
/// `left_px(x)`, `top_px(y)`, `z(50)`, `keep_in_view()`.
#[track_caller]
pub fn at_window(x: f32, y: f32, modifier: Modifier, content: impl FnOnce()) {
    detached_at(x, y, MENU_LAYER, modifier, content);
}

/// The tooltip the pointer rests on: where its box is, the words, and when the pointer came to it.
#[derive(Clone, Debug, PartialEq)]
struct Resting {
    /// Where the box is in the window.
    rect: Rect,
    /// The window the box is in.
    window: report::WindowKey,
    /// The words.
    words: String,
    /// When the pointer came to it, on the frame's clock.
    since: f32,
}

thread_local! {
    /// The tooltip the pointer rests on, if any.
    static RESTING: RefCell<Option<Resting>> = const { RefCell::new(None) };
    /// How long the pointer rests on a box before its tooltip shows, in milliseconds.
    static DELAY: Cell<f32> = const { Cell::new(300.0) };
}

/// Forgets the tooltip the pointer rested on, for a window drawn afresh, such as the next snapshot.
pub fn reset_tooltip() {
    RESTING.with(|resting| *resting.borrow_mut() = None);
}

/// Shows tooltips once the pointer has rested `ms` milliseconds on their box; snapshots use nought.
pub fn set_tooltip_delay(ms: f32) {
    DELAY.with(|delay| delay.set(ms));
}

/// Modifiers for words shown over an area once the pointer rests on it.
pub trait Tooltip: Sized {
    /// Shows `words` over the area, above everything else, once the pointer
    /// has rested on it for a moment: `.tooltip(words)`. The screen's
    /// [`tooltips`] draws it.
    fn tooltip(self, words: impl Into<String>) -> Self;
}

impl Tooltip for Modifier {
    fn tooltip(self, words: impl Into<String>) -> Self {
        let words: String = words.into();
        let rect = Rc::new(Cell::new((Rect::default(), report::MAIN)));
        let placed = rect.clone();
        self.on_placed(move |at, _| placed.set((at, report::painting())))
            .on_hover(move |inside| {
                RESTING.with(|resting| {
                    let mut resting = resting.borrow_mut();
                    let mine = resting.as_ref().is_some_and(|held| held.words == words);
                    if inside {
                        let (rect, window) = rect.get();
                        *resting = Some(Resting {
                            rect,
                            window,
                            words: words.clone(),
                            since: frame::clock_ms(),
                        });
                    } else if mine {
                        *resting = None;
                    }
                });
                frame::request_compose();
            })
    }
}

/// Emits the tooltip the pointer rests on in the window `window`, once it
/// has rested long enough, above its box or below it where there is no room
/// above, over everything. Call it last in a window's root.
#[track_caller]
pub fn tooltips(window: report::WindowKey) {
    let Some(resting) = RESTING
        .with(|resting| resting.borrow().clone())
        .filter(|resting| resting.window == window)
    else {
        return;
    };
    let due = resting.since + DELAY.with(Cell::get);
    if frame::clock_ms() < due {
        frame::wake_at(due);
        frame::request_compose();
        return;
    }
    let kind = Type::new(12).ratio(1.4);
    let height = resting.words.lines().count().max(1) as f32 * kind.line_height() + 6.0 + 2.0;
    let above = resting.rect.origin.y - TOOLTIP_GAP - height;
    let top = if above >= 0.0 {
        above
    } else {
        resting.rect.origin.y + resting.rect.size.height + TOOLTIP_GAP
    };
    detached_at(
        resting.rect.origin.x,
        top,
        TOOLTIP_LAYER,
        Modifier::new(),
        || {
            column(
                Modifier::new()
                    .fill_rounded(Role::Surface, Radius::Sm)
                    .stroke(Role::Border, Radius::Sm)
                    .pad_x(1.5)
                    .pad_y(0.75),
                Flex::new().start(),
                || {
                    for line in resting.words.lines() {
                        label(line.to_owned(), kind.tone(Role::Text));
                    }
                },
            );
        },
    );
}

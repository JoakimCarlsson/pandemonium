//! Scrolling boxes whose offset lives in the model.
//!
//! The old boxes scrolled by `scroll_from_start(id, offset)` and
//! `scroll_from_end(id, back)`: the model keeps how far each box is
//! scrolled, and the frame reported how far it could scroll. [`scroll_area`]
//! does the same with its own layout instead of rustle's `ScrollState`,
//! because the offset must stay in the model, where fixtures, snapshots and
//! the logic that keeps a caret or a row in view read and write it: the box
//! measures its content without a bound down its length, shows it moved up
//! by the model's offset kept within the room, clips it, records the room in
//! the frame's [`Report`](crate::kit::Report), turns the wheel and drags of
//! its bar into [`Action::Scroll`](crate::kit::Action::Scroll), and draws a
//! bar as the old one looked: thin, widening under the pointer and while held.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use rustle::{
    BoxAlignment, Children, Constraints, Layout, LayoutResult, Modifier, Offset, PointerButton,
    PointerInputPhase, PointerPhase, Size,
};

use crate::kit::bus::{self, Action};
use crate::kit::focus::FocusId;
use crate::kit::frame;
use crate::kit::report::{self, Scroller};
use crate::kit::theme::{self, Role, Tone};

/// How wide the bar of a scrolling box is, in logical pixels.
const BAR_WIDTH: f32 = 4.0;

/// How far the bar stays from the box's edges, in logical pixels.
const BAR_INSET: f32 = 3.0;

/// How short the bar is at least, in logical pixels.
const BAR_MINIMUM: f32 = 24.0;

/// How wide the bar grows while the pointer is on it or holds it, in logical pixels.
const BAR_WIDE: f32 = 7.0;

/// How wide the strip along the right edge is where the pointer takes hold of the bar, in logical pixels.
pub const SCROLLBAR_GRIP: f32 = 14.0;

/// Which end of the content the model's position is measured from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Anchor {
    /// The top: the position is how far down the content is scrolled: `scroll_from_start`.
    Start,
    /// The bottom: the position is how far back up from the end it is: `scroll_from_end`.
    End,
}

/// What one layout of a scrolling box found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Measure {
    /// How far its content can scroll.
    room: f32,
    /// How far from the top it is scrolled, kept within the room.
    offset: f32,
    /// How tall it is.
    viewport: f32,
}

thread_local! {
    /// What the latest layout of each scrolling box found, by id.
    static MEASURED: RefCell<HashMap<FocusId, Measure>> = RefCell::new(HashMap::new());
}

/// How far the scrolling box `id` could scroll when it was last laid out.
pub fn room_of(id: FocusId) -> f32 {
    MEASURED.with(|measured| measured.borrow().get(&id).map_or(0.0, |held| held.room))
}

/// A scrolling box being described.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    /// What names it.
    id: FocusId,
    /// Which end the position is measured from.
    anchor: Anchor,
    /// How far it is scrolled from that end, in logical pixels.
    position: f32,
    /// Whether it draws a bar.
    bar: bool,
}

impl Scroll {
    /// A box named `id` scrolled `offset` pixels down from the top of its content: `.scroll_from_start(id, offset)`.
    pub fn from_start(id: FocusId, offset: f32) -> Self {
        Self {
            id,
            anchor: Anchor::Start,
            position: offset,
            bar: true,
        }
    }

    /// A box named `id` scrolled to the bottom of its content, then `back` pixels back up: `.scroll_from_end(id, back)`.
    pub fn from_end(id: FocusId, back: f32) -> Self {
        Self {
            id,
            anchor: Anchor::End,
            position: back,
            bar: true,
        }
    }

    /// The box without a bar.
    pub fn without_bar(mut self) -> Self {
        self.bar = false;
        self
    }
}

/// Lays the content out with no bound down its length and moves it up by the offset kept within
/// the room, which is rounded to whole pixels as the edges it is measured between are.
struct Scrolled {
    /// The box.
    scroll: Scroll,
    /// Where what the layout found is put for the bar and the report.
    out: Rc<Cell<Measure>>,
}

impl Layout for Scrolled {
    fn measure(&self, children: &mut Children, constraints: Constraints) -> LayoutResult {
        let open = Constraints {
            min_width: constraints.min_width,
            max_width: constraints.max_width,
            min_height: 0.0,
            max_height: f32::INFINITY,
        };
        let content = if children.is_empty() {
            Size::new(0.0, 0.0)
        } else {
            children.measure(0, open)
        };
        let height = if constraints.max_height.is_finite() {
            content
                .height
                .min(constraints.max_height)
                .max(constraints.min_height)
        } else {
            content.height.max(constraints.min_height)
        };
        let width = content
            .width
            .max(constraints.min_width)
            .min(constraints.max_width);
        let room = (content.height - height).max(0.0).round();
        let position = self.scroll.position.clamp(0.0, room);
        let offset = match self.scroll.anchor {
            Anchor::Start => position,
            Anchor::End => room - position,
        };
        let measure = Measure {
            room,
            offset,
            viewport: height,
        };
        self.out.set(measure);
        MEASURED.with(|measured| measured.borrow_mut().insert(self.scroll.id, measure));
        let mut positions = vec![Offset::new(0.0, -offset)];
        positions.truncate(children.len());
        LayoutResult {
            size: Size::new(width, height),
            positions,
        }
    }
}

/// Emits a box with `modifier` that shows `content` scrolled as `scroll` says.
///
/// Give it a bounded height (a fixed height, `max_h_px`, or `grow` in a
/// column of bounded height): without one it is as tall as its content and
/// does not scroll. The content is one child: wrap several in a column.
#[track_caller]
pub fn scroll_area(modifier: Modifier, scroll: Scroll, content: impl FnOnce()) {
    let id = scroll.id;
    let out = Rc::new(Cell::new(Measure::default()));
    let (placed, wheel, bar) = (out.clone(), out.clone(), out.clone());
    let previous = room_of(id);
    let outer = modifier
        .clip()
        .on_placed(move |rect, _| {
            let measure = placed.get();
            report::record(|report| {
                report.scrollers.push(Scroller {
                    id,
                    rect,
                    room: measure.room,
                    offset: measure.offset,
                })
            });
        })
        .pointer_input(move |event| {
            if event.phase != PointerInputPhase::Scroll {
                return false;
            }
            let room = wheel.get().room;
            if room <= 0.0 {
                return false;
            }
            bus::send(Action::Scroll {
                id,
                pixels: -event.scroll_delta.y,
                room,
            });
            true
        });
    rustle::stack_with(outer, move || {
        rustle::layout(
            Modifier::new().fill_max_width(),
            Scrolled { scroll, out },
            content,
        );
        if scroll.bar && previous > 0.0 {
            scrollbar(id, bar);
        }
    });
}

/// Emits the strip along the right edge of a scrolling box that draws its
/// bar and turns drags of it into scrolling.
#[track_caller]
fn scrollbar(id: FocusId, measure: Rc<Cell<Measure>>) {
    let held = Rc::new(Cell::new(None::<f32>));
    let (drawn, dragged) = (measure.clone(), measure);
    let (holding, grip) = (held.clone(), held);
    let theme = theme::theme();
    let strip = Modifier::new()
        .width(SCROLLBAR_GRIP)
        .fill_max_height()
        .align(BoxAlignment::TopEnd)
        .on_pointer_button(PointerButton::Primary, move |event| {
            let y = event.window_position.y;
            match event.phase {
                PointerPhase::Press => grip.set(Some(y)),
                PointerPhase::Drag => {
                    let Some(last) = grip.get() else {
                        return;
                    };
                    let measure = dragged.get();
                    let (_, thumb, track) = thumb_of(measure);
                    let travel = (track - thumb).max(1.0);
                    grip.set(Some(y));
                    bus::send(Action::Scroll {
                        id,
                        pixels: (y - last) * measure.room / travel,
                        room: measure.room,
                    });
                }
                PointerPhase::Release | PointerPhase::Click => grip.set(None),
            }
        })
        .draw_behind_interactive(move |canvas, interaction| {
            let measure = drawn.get();
            if measure.room <= 0.0 {
                return;
            }
            let held = holding.get().is_some() || frame::current().held_bar == Some(id);
            let (width, tone, alpha) = if held {
                (BAR_WIDE, Role::Text, 0.6)
            } else if interaction.hovered {
                (BAR_WIDE, Role::Muted, 0.85)
            } else {
                (BAR_WIDTH, Role::Muted, 0.55)
            };
            let (top, thumb, _) = thumb_of(measure);
            let color = theme::color_in(&theme, Tone::from(tone).alpha(alpha));
            let left = canvas.width() - BAR_INSET - width;
            canvas.rounded_rect(left, top, width, thumb, width / 2.0, color);
        });
    rustle::spacer(strip);
}

/// Where the bar of a box `measure` describes starts, how long it is, and how long its track is.
fn thumb_of(measure: Measure) -> (f32, f32, f32) {
    let height = measure.viewport;
    let track = (height - 2.0 * BAR_INSET).max(0.0);
    let thumb = (track * height / (height + measure.room))
        .max(BAR_MINIMUM)
        .min(track);
    let share = if measure.room > 0.0 {
        (measure.offset / measure.room).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (BAR_INSET + (track - thumb) * share, thumb, track)
}

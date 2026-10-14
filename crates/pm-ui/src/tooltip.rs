//! The tooltip: a few words shown over a box once the pointer has rested on it for a moment, painted over everything else.

use std::cell::RefCell;

use pm_renderer::Rect;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, items_start, leading_ratio, px, py, rounded,
    text_color, text_px,
};

use crate::node::{Node, node, text};

/// How long the pointer rests on a box before its tooltip shows, in milliseconds.
const DELAY_MS: f32 = 300.0;

/// How far a tooltip stays from the box it is about, in logical pixels.
pub(crate) const GAP: f32 = 4.0;

/// The tooltip the pointer rests on: the box, the words and when the pointer came to rest on it.
struct Held {
    /// Where the box is.
    rect: Rect,
    /// The words.
    words: String,
    /// When the pointer came to it, in milliseconds since the application started.
    since: f32,
}

thread_local! {
    /// The tooltip asked for in the frames before this one, while it goes on being asked for.
    static HELD: RefCell<Option<Held>> = const { RefCell::new(None) };
}

/// Where the tooltip asked for in a frame stands.
pub(crate) enum Showing {
    /// No box under the pointer asks for one.
    Nothing,
    /// One is asked for, and the pointer has yet to rest on its box long enough.
    Waiting,
    /// The words to show over the box at the rectangle.
    Shown(Rect, String),
}

/// Where the tooltip `asked` for over a box in the frame drawn at `now` stands, counting how long it has been asked for over the same box.
pub(crate) fn settle(asked: Option<(Rect, String)>, now: f32) -> Showing {
    HELD.with(|held| {
        let mut held = held.borrow_mut();
        let Some((rect, words)) = asked else {
            *held = None;
            return Showing::Nothing;
        };
        let since = held
            .as_ref()
            .filter(|kept| kept.rect == rect && kept.words == words && kept.since <= now)
            .map_or(now, |kept| kept.since);
        *held = Some(Held {
            rect,
            words: words.clone(),
            since,
        });
        match now - since >= DELAY_MS {
            true => Showing::Shown(rect, words),
            false => Showing::Waiting,
        }
    })
}

/// The tooltip saying `words`, in a box that holds it at its own size: a small bordered card.
pub(crate) fn holder(words: &str) -> Node {
    node().styles([flex(), items_start()]).child(
        node()
            .styles([
                px(1.5),
                py(0.75),
                rounded(Radius::Sm),
                border(),
                border_color(Role::Border),
                bg(Role::Surface),
            ])
            .child(text(words.to_owned()).styles([
                text_px(12),
                leading_ratio(1.4),
                text_color(Role::Text),
            ])),
    )
}

//! The ring in the prompt box that shows how full the conversation's context is, and the menu it opens.

use pm_acp::Usage;
use pm_input::FocusId;
use pm_style::{
    Role, bg, cursor_pointer, flex, flex_col, flex_none, gap, h, h_px, items_center,
    justify_between, justify_center, overflow_hidden, px, py, rounded_full, w_frac, w_px,
};
use pm_ui::{Node, node};

use crate::Popover;
use crate::parts::line;
use crate::popover::heading;
use crate::sample::Session;
use crate::state::{CONTEXT, MENU_BODY};

/// How wide the context menu is, in logical pixels.
const MENU_WIDTH: f32 = 320.0;

/// How wide a ring is, in logical pixels.
const RING: f32 = 20.0;

/// How thick a ring is, in logical pixels.
const RING_WIDTH: f32 = 2.5;

/// The share used from which a meter is drawn as a danger.
const DANGEROUS: u32 = 90;

/// How full the context is, as a whole percentage, when the agent has said.
fn context_percent(context: Option<&Usage>) -> Option<u32> {
    context
        .filter(|usage| usage.size > 0)
        .map(|usage| (usage.used.saturating_mul(100) / usage.size).min(100) as u32)
}

/// The tone a meter that is `used` percent full is drawn in.
fn tone(used: u32) -> Role {
    if used >= DANGEROUS {
        Role::Danger
    } else {
        Role::Accent
    }
}

/// The button that opens the menu of the context: a ring filled as far as the context is.
pub fn context_button(session: &Session, open: bool) -> Option<Node> {
    session
        .context
        .map(|_| button(context_percent(session.context.as_ref()), open, CONTEXT))
}

/// A round button named `id` holding a ring filled `used` percent.
fn button(used: Option<u32>, open: bool, id: FocusId) -> Node {
    let mut button = node().styles([
        flex(),
        flex_none(),
        items_center(),
        justify_center(),
        w_px(32),
        h_px(32),
        rounded_full(),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ]);
    if open {
        button = button.styles([bg(Role::SurfaceSelected)]);
    }
    button.child(ring(used)).focusable(id)
}

/// A ring that fills clockwise from the top as `used` percent grows.
fn ring(used: Option<u32>) -> Node {
    let share = used.unwrap_or(0);
    node().styles([flex_none(), w_px(RING), h_px(RING)]).arc(
        share as f32 / 100.0,
        RING_WIDTH,
        tone(share),
        Role::Border,
    )
}

/// The menu over the context button: how full the context is, and in tokens.
pub fn context_menu(session: &Session) -> Node {
    let caption = session.context.map_or_else(
        || "Reported once the agent has taken a turn".to_owned(),
        |usage| {
            format!(
                "{} of {} tokens",
                thousands(usage.used),
                thousands(usage.size)
            )
        },
    );
    Popover::new()
        .width(MENU_WIDTH)
        .child(heading("Context", None))
        .child(meter(
            "Context window",
            context_percent(session.context.as_ref()),
            &caption,
        ))
        .over_button_end(MENU_BODY)
}

/// A name with how much is used at its right, a bar under them and a caption under that.
fn meter(name: &str, used: Option<u32>, caption: &str) -> Node {
    let share = used.unwrap_or(0).min(100);
    let mut track = node().styles([
        h(1.5),
        rounded_full(),
        bg(Role::SurfaceActive),
        overflow_hidden(),
    ]);
    if share > 0 {
        track = track.child(node().styles([
            h(1.5),
            w_frac(share, 100),
            rounded_full(),
            bg(tone(share)),
        ]));
    }
    let mut section = node()
        .styles([flex(), flex_col(), gap(1.5), px(2.5), py(2)])
        .child(
            node()
                .styles([flex(), items_center(), justify_between()])
                .child(line(name, 14, 1.4, Role::Text))
                .child(line(
                    used.map_or_else(|| "No data".to_owned(), |used| format!("{used}% used")),
                    13,
                    1.4,
                    Role::Muted,
                )),
        )
        .child(track);
    if !caption.is_empty() {
        section = section.child(line(caption, 13, 1.4, Role::Subtle));
    }
    section
}

/// A count of tokens in whole thousands once it reaches a thousand.
fn thousands(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        _ => format!("{}k", count / 1000),
    }
}

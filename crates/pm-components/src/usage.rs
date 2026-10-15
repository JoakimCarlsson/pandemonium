//! The ring in the prompt box that shows how full the conversation's context is, and the words it says when the pointer rests on it.

use pm_acp::Usage;
use pm_style::{
    Color, Role, bg, cursor_default, flex, flex_none, h_px, items_center, justify_center, rounded_full,
    w_px,
};
use pm_ui::{Node, node};

use crate::sample::Session;

/// How wide a ring is, in logical pixels.
const RING: f32 = 20.0;

/// How thick a ring is, in logical pixels.
const RING_WIDTH: f32 = 2.5;

/// The hue of an empty ring, in degrees: green.
const EMPTY_HUE: f32 = 145.0;

/// The hue of a full ring, in degrees: red.
const FULL_HUE: f32 = 25.0;

/// How full the context is, as a whole percentage, when the agent has said.
fn context_percent(context: &Usage) -> Option<u32> {
    (context.size > 0)
        .then(|| (context.used.saturating_mul(100) / context.size).min(100) as u32)
}

/// The colour of a ring that is `used` percent full: green when empty, through amber, to red when full.
fn tone(used: u32) -> Color {
    let share = used.min(100) as f32 / 100.0;
    Color::oklch(0.72, 0.17, EMPTY_HUE + (FULL_HUE - EMPTY_HUE) * share)
}

/// The ring that shows how full the context is, with the figures as its tooltip, once the agent has said.
pub fn context_ring(session: &Session) -> Option<Node> {
    let context = session.context.as_ref()?;
    let used = context_percent(context);
    Some(
        node()
            .styles([
                flex(),
                flex_none(),
                items_center(),
                justify_center(),
                w_px(32),
                h_px(32),
                rounded_full(),
                cursor_default(),
                bg(Role::SurfaceHover).hover(),
            ])
            .child(ring(used))
            .tooltip(caption(context, used)),
    )
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

/// What the ring says: how full the context is and in tokens.
fn caption(context: &Usage, used: Option<u32>) -> String {
    let tokens = format!(
        "{} of {} tokens",
        thousands(context.used),
        thousands(context.size)
    );
    match used {
        Some(used) => format!("Context {used}% used · {tokens}"),
        None => format!("Context {tokens}"),
    }
}

/// A count of tokens in whole thousands once it reaches a thousand.
fn thousands(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        _ => format!("{}k", count / 1000),
    }
}

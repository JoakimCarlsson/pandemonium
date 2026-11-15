//! The shadow the window casts around its self-drawn frame while it floats: the layers it
//! is made of, handed to Rustle, which shows it beside the window where the desktop lets
//! it, and the band the window draws it in itself where the desktop wants it inside.

use rustle::prelude::*;
use rustle::{ShadowLayer, WindowShadow};

/// How far the shadow of a floating window reaches past its frame, in logical pixels.
pub const SHADOW_MARGIN: f32 = 32.0;

/// The radius of the corners of a floating window, in logical pixels.
pub const FRAME_RADIUS: f32 = 10.0;

/// The layers of the shadow a floating window casts: a wide soft one that falls away below
/// it, a tighter one, and a thin one that keeps its edge defined.
pub const WINDOW_SHADOW: [ShadowLayer; 3] = [
    ShadowLayer::new(0.0, 8.0, 22.0, 0.0, 0.5),
    ShadowLayer::new(0.0, 2.0, 8.0, 0.0, 0.32),
    ShadowLayer::new(0.0, 0.0, 2.0, 0.0, 0.28),
];

/// The shadow the main window casts, as Rustle's window options take it.
pub fn window_shadow() -> WindowShadow {
    WindowShadow {
        margin: SHADOW_MARGIN,
        radius: FRAME_RADIUS,
        layers: WINDOW_SHADOW.to_vec(),
    }
}

/// Emits `content` filling the window, inside a band `band` logical pixels wide that holds
/// the window's shadow, with the frame's corners rounded while the window is `floating`;
/// `content` alone, square, when the window is maximized or full screen. Without a band, as
/// where the shadow is shown beside the window, the corners are still rounded so they meet
/// the shadow drawn around them.
pub fn shadow_band(band: f32, floating: bool, content: impl FnOnce()) {
    if !floating {
        content();
        return;
    }
    if band <= 0.0 {
        stack_with(
            Modifier::new().fill_max_size().clip_rounded(FRAME_RADIUS),
            || {
                content();
                frame_edge();
            },
        );
        return;
    }
    let shadowed = WINDOW_SHADOW.iter().fold(
        Modifier::new().fill_max_size().padding(band),
        |modifier, layer| {
            modifier.drop_shadow(
                layer.x,
                layer.y,
                layer.blur,
                Color::rgba(0, 0, 0, (layer.alpha * 255.0).round() as u8),
                FRAME_RADIUS,
            )
        },
    );
    stack_with(shadowed.clip_rounded(FRAME_RADIUS), || {
        content();
        frame_edge();
    });
}

/// The hairline drawn around the edge of a floating frame, over what it holds, so the window stands apart from what is behind it.
fn frame_edge() {
    let scheme = rustle::color_scheme();
    spacer(
        Modifier::new()
            .fill_max_size()
            .border(1.0, scheme.outline_variant, FRAME_RADIUS),
    );
}

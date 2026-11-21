//! Words shown over a control once the pointer rests on it, on rustle's tooltip area: our
//! box, kept to the left edge of the control above it, or below it where there is no room.

use crate::theme;
use crate::theme::Frame;
use rustle::{Alignment, Arrangement, TextOverflow};
use rustle::{CompositionLocal, Modifier, Offset, PopupPositionProvider, Rect, Size, tooltip_area};

use crate::theme::{Radius, Role};

/// How far a tooltip's box stays above the box it is about, in logical pixels.
const ABOVE: f32 = 6.0;

/// How far a tooltip's box stays below the box it is about, when there is no room above.
const BELOW: f32 = 4.0;

/// How long the pointer rests on a box before its tooltip shows, in milliseconds.
const DELAY: u64 = 300;

/// How long the pointer rests on a box before its tooltip shows in what is composed below;
/// snapshots show tooltips at once.
static LOCAL_TOOLTIP_DELAY: CompositionLocal<u64> = CompositionLocal::new(|| DELAY);

/// Composes `content` with tooltips shown once the pointer has rested `millis` milliseconds.
pub fn provide_tooltip_delay<R>(millis: u64, content: impl FnOnce() -> R) -> R {
    LOCAL_TOOLTIP_DELAY.provide(millis, content)
}

/// Places a tooltip with its left edge on its box's, above the box, or below it without room
/// above inside the window's frame, kept inside the frame clear of the band that holds the
/// window's shadow, so it never covers its own box.
struct TooltipPlacement;

impl PopupPositionProvider for TooltipPlacement {
    /// Above or below the box, at its left edge, moved inside the band.
    fn calculate_position(&self, anchor: Rect, window: Size, popup: Size) -> Offset {
        let band = crate::shadow_margin();
        let above = anchor.origin.y - ABOVE - popup.height;
        let y = if above >= band {
            above
        } else {
            anchor.origin.y + anchor.size.height + BELOW
        };
        let inside = |at: f32, length: f32, room: f32| at.min(room - band - length).max(band);
        Offset::new(
            inside(anchor.origin.x, popup.width, window.width),
            inside(y, popup.height, window.height),
        )
    }
}

/// Emits `content` and, once the pointer has rested on it for a moment, `words` in a small
/// box over it, above everything else: Compose Desktop's `TooltipArea`.
#[track_caller]
pub fn with_tooltip(words: impl Into<String>, content: impl FnOnce()) {
    with_tooltip_in(Modifier::new(), words, content);
}

/// Like [`with_tooltip`], the area holding `content` laid out with `modifier`, such as the
/// weight a control filling its row had.
#[track_caller]
pub fn with_tooltip_in(modifier: Modifier, words: impl Into<String>, content: impl FnOnce()) {
    let words: String = words.into();
    tooltip_area(
        move || {
            let kind = theme::text_style(12, 1.4).color(theme::color(Role::Text));
            let inner = Modifier::new()
                .panel(Radius::Sm)
                .padding_xy(theme::sp(1.5), theme::sp(0.75));
            rustle::column_arranged(inner, Arrangement::Start, Alignment::Start, || {
                for line in words.lines() {
                    rustle::basic_text(
                        Modifier::new(),
                        line.to_owned(),
                        kind,
                        TextOverflow::Ellipsis,
                    );
                }
            });
        },
        modifier,
        LOCAL_TOOLTIP_DELAY.current(),
        TooltipPlacement,
        content,
    );
}

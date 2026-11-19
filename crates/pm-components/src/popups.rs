//! What floats over the screen, on rustle popups: popovers placed against what opened them,
//! and menus opened at a point of the window, kept inside the window and clear of the band
//! around it that holds its shadow. A popup a press elsewhere closes asks the app through
//! its `on_dismiss_request`.

use std::rc::Rc;

use rustle::{
    Modifier, Offset, PopupPositionProvider, PopupProperties, Rect, Size,
    popup_position_provider_at_position, popup_with,
};

use crate::events;

/// Where a popover opens against what opened it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    /// Above it, their left edges lined up.
    Above,
    /// Above it, their right edges lined up.
    AboveEnd,
    /// Below it, their left edges lined up.
    Below,
    /// Below it, their right edges lined up.
    BelowEnd,
}

/// Places a popover against its anchor as a [`Placement`] says, `gap` logical pixels away.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Against {
    /// Where the popover goes.
    placement: Placement,
    /// The space between the anchor and the popover, in logical pixels.
    gap: f32,
}

impl PopupPositionProvider for Against {
    /// Above or below the anchor, lined up with its left or right edge.
    fn calculate_position(&self, anchor: Rect, _: Size, popup: Size) -> Offset {
        let x = match self.placement {
            Placement::Above | Placement::Below => anchor.origin.x,
            Placement::AboveEnd | Placement::BelowEnd => {
                anchor.origin.x + anchor.size.width - popup.width
            }
        };
        let y = match self.placement {
            Placement::Above | Placement::AboveEnd => anchor.origin.y - popup.height - self.gap,
            Placement::Below | Placement::BelowEnd => {
                anchor.origin.y + anchor.size.height + self.gap
            }
        };
        Offset::new(x, y)
    }
}

/// What a popup tells the app when a press lands outside it, when a press elsewhere closes
/// it: the app works out what that press closes.
fn dismiss_request(dismissed_elsewhere: bool) -> Option<Rc<dyn Fn()>> {
    dismissed_elsewhere.then(|| {
        let operations = events::operations();
        Rc::new(move || operations.dismiss_requested()) as Rc<dyn Fn()>
    })
}

/// How a popup of the app behaves: a press outside it asks the app to dismiss it when
/// `dismissed_elsewhere`; Escape is the app's own key.
fn properties(dismissed_elsewhere: bool) -> PopupProperties {
    PopupProperties {
        focusable: false,
        dismiss_on_back_press: false,
        dismiss_on_click_outside: dismissed_elsewhere,
        clipping_enabled: true,
    }
}

/// Emits `trigger` in a box with `modifier` and, while `popover` is given, the popover as a
/// popup placed against the box as `placement` says, `gap` style steps away, over
/// everything and kept inside the window. When `dismissed_elsewhere`, a press outside the
/// popover asks the app to close what a press elsewhere closes.
#[track_caller]
pub fn anchored(
    modifier: Modifier,
    placement: Placement,
    gap: f32,
    dismissed_elsewhere: bool,
    trigger: impl FnOnce(),
    popover: Option<impl FnOnce()>,
) {
    let gap = crate::theme::sp(gap);
    rustle::stack_with(modifier, move || {
        trigger();
        if let Some(popover) = popover {
            popup_with(
                Against { placement, gap },
                dismiss_request(dismissed_elsewhere),
                properties(dismissed_elsewhere),
                popover,
            );
        }
    });
}

/// Emits `content` as a popup with its top left at (`x`, `y`) in the window, over
/// everything, kept inside the window's frame clear of the band around it that holds its
/// shadow, where a press resizes the window instead of reaching it; taking no room where it
/// is declared. When `dismissed_elsewhere`, a press outside it asks the app to close what a
/// press elsewhere closes: a menu opened where the pointer was.
#[track_caller]
pub fn at_window(x: f32, y: f32, dismissed_elsewhere: bool, content: impl FnOnce()) {
    popup_with(
        popup_position_provider_at_position(Offset::new(x, y), crate::shadow_margin()),
        dismiss_request(dismissed_elsewhere),
        properties(dismissed_elsewhere),
        content,
    );
}

/// Emits `content` as a popup placed `offset` logical pixels from the top left of the box it
/// is declared in, over everything and kept inside the window. When `dismissed_elsewhere`,
/// a press outside it asks the app to close what a press elsewhere closes.
#[track_caller]
pub fn in_box_at(offset: (f32, f32), dismissed_elsewhere: bool, content: impl FnOnce()) {
    popup_with(
        rustle::AlignmentOffsetPositionProvider {
            alignment: rustle::BoxAlignment::TopStart,
            offset: Offset::new(offset.0, offset.1),
        },
        dismiss_request(dismissed_elsewhere),
        properties(dismissed_elsewhere),
        content,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A popover above its anchor at the end lines up the right edges, `gap` above.
    #[test]
    fn a_popover_above_the_end_lines_up_right_edges() {
        let against = Against {
            placement: Placement::AboveEnd,
            gap: 2.0,
        };
        let at = against.calculate_position(
            Rect::new(Offset::new(100.0, 200.0), Size::new(40.0, 20.0)),
            Size::new(800.0, 600.0),
            Size::new(120.0, 50.0),
        );
        assert_eq!(at, Offset::new(20.0, 148.0));
    }
}

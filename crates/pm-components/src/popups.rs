//! What floats over the screen, on rustle popups: popovers placed against what opened them,
//! menus opened at a point of the window or hanging from the control they are declared in, kept
//! inside the window and clear of the band around it that holds its shadow. Each popup that
//! closes closes itself through its own `on_dismiss_request`, as its [`Dismissal`] says.

use std::rc::Rc;

use rustle::{
    DropdownMenuPositionProvider, Modifier, Offset, PopupPositionProvider, PopupProperties,
    PositionAtPoint, Rect, Size, popup_position_provider_at_position, popup_with,
};

use crate::events;
use crate::state::WorkspaceState;
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

/// Which side of a line of text a popup hangs on when there is room there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    /// Under the line; above it when it would run past the bottom of the window.
    Below,
    /// Over the line; under it when it would run past the top of the window.
    Above,
}

/// Places a popup against a line of text of the window, by the popup's own size: under or
/// over the line as `side` says, `gap` logical pixels from it, its left edge at the line's
/// point, and kept inside the window's frame as a popup at a point is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ByLine {
    /// The left edge of the place on the line and the top of the line, in the window.
    pub at: (f32, f32),
    /// How tall the line is, in logical pixels.
    pub height: f32,
    /// The space between the line and the popup, in logical pixels.
    pub gap: f32,
    /// The side the popup hangs on when there is room there.
    pub side: Side,
}

impl PopupPositionProvider for ByLine {
    /// Under or over the line, flipped to the other side when the popup does not fit, then
    /// kept inside the window's frame.
    fn calculate_position(&self, anchor: Rect, window: Size, popup: Size) -> Offset {
        let (x, top) = self.at;
        let below = top + self.height + self.gap;
        let above = top - popup.height - self.gap;
        let y = match self.side {
            Side::Below if below + popup.height > window.height => above.max(0.0),
            Side::Below => below,
            Side::Above if above >= 0.0 => above,
            Side::Above => below,
        };
        at_point((x, y), (0.0, 0.0)).calculate_position(anchor, window, popup)
    }
}

/// Places a popup beside its anchor, `gap` logical pixels past its right edge with their top
/// edges together, kept inside the window's frame as a popup at a point is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beside {
    /// The space between the anchor and the popup, in logical pixels.
    pub gap: f32,
}

impl PopupPositionProvider for Beside {
    /// Past the anchor's right edge, at its top.
    fn calculate_position(&self, anchor: Rect, window: Size, popup: Size) -> Offset {
        let at = (
            anchor.origin.x + anchor.size.width + self.gap,
            anchor.origin.y,
        );
        at_point(at, (0.0, 0.0)).calculate_position(anchor, window, popup)
    }
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

/// How a popup is dismissed: Compose's `PopupProperties` with its `onDismissRequest`.
#[derive(Clone)]
pub enum Dismissal {
    /// Nothing dismisses it: it stays until the state that shows it changes, and lets every
    /// press outside it through, as a card over the text or a drag's ghost does.
    Never,
    /// A focusable popup, as Compose's menus and popovers are: Escape and a press outside it
    /// call the lambda, and the press goes no further.
    Focusable(Rc<dyn Fn()>),
    /// A popup that is not focusable, as a box typed in over the text is: a press outside it
    /// calls the lambda and goes on to what it landed on, and the keys stay with what holds
    /// focus.
    Outside(Rc<dyn Fn()>),
}

impl Dismissal {
    /// A focusable popup that `change` closes, run on the workspace when Escape or a press
    /// outside it asks it to be dismissed.
    pub fn focusable(change: impl Fn(WorkspaceState) -> WorkspaceState + 'static) -> Self {
        Self::Focusable(closing(change))
    }

    /// A popup that is not focusable that `change` closes, run on the workspace when a press
    /// outside it asks it to be dismissed.
    pub fn outside(change: impl Fn(WorkspaceState) -> WorkspaceState + 'static) -> Self {
        Self::Outside(closing(change))
    }

    /// The popup's properties and the lambda its dismissal calls, if any.
    fn split(self) -> (PopupProperties, Option<Rc<dyn Fn()>>) {
        let (focusable, on_dismiss_request) = match self {
            Self::Never => (false, None),
            Self::Focusable(dismiss) => (true, Some(dismiss)),
            Self::Outside(dismiss) => (false, Some(dismiss)),
        };
        let dismissed = on_dismiss_request.is_some();
        let properties = PopupProperties {
            focusable,
            dismiss_on_back_press: dismissed,
            dismiss_on_click_outside: dismissed,
            clipping_enabled: true,
        };
        (properties, on_dismiss_request)
    }
}

/// The lambda that runs `change` on the workspace, through the operations the view is
/// composed under: what a popup's `on_dismiss_request` does to close it.
fn closing(change: impl Fn(WorkspaceState) -> WorkspaceState + 'static) -> Rc<dyn Fn()> {
    let operations = events::operations();
    let change = Rc::new(change);
    Rc::new(move || {
        let change = Rc::clone(&change);
        operations.update(Box::new(move |state| change(state)));
    })
}

/// Emits `content` as a popup where `placed` puts it, over everything and kept inside the
/// window, dismissed as `dismissal` says: Compose's
/// `Popup(popupPositionProvider, onDismissRequest, properties)`.
#[track_caller]
pub fn float(
    placed: impl PopupPositionProvider + 'static,
    dismissal: Dismissal,
    content: impl FnOnce(),
) {
    let (properties, on_dismiss_request) = dismissal.split();
    popup_with(placed, on_dismiss_request, properties, content);
}

/// Emits `trigger` in a box with `modifier` and, while `popover` is given, the popover as a
/// popup placed against the box as `placement` says, `gap` style steps away, over
/// everything and kept inside the window, dismissed as `dismissal` says. A focusable popover
/// takes the press on the box that dismisses it, so the box's controls do not open it again.
#[track_caller]
pub fn anchored(
    modifier: Modifier,
    placement: Placement,
    gap: f32,
    dismissal: Dismissal,
    trigger: impl FnOnce(),
    popover: Option<impl FnOnce()>,
) {
    let gap = crate::theme::sp(gap);
    rustle::stack_with(modifier, move || {
        trigger();
        if let Some(popover) = popover {
            float(Against { placement, gap }, dismissal, popover);
        }
    });
}

/// Emits `content` as a popup with its top left at (`x`, `y`) in the window moved by
/// `offset`, over everything, kept inside the window's frame clear of the band around it
/// that holds its shadow, where a press resizes the window instead of reaching it; taking
/// no room where it is declared, and dismissed as `dismissal` says: Compose's popup placed
/// by `rememberPopupPositionProviderAtPosition`, for a menu opened where the pointer was.
#[track_caller]
pub fn at_window(at: (f32, f32), offset: (f32, f32), dismissal: Dismissal, content: impl FnOnce()) {
    float(at_point(at, offset), dismissal, content);
}

/// Places a popup with its top left at `at` in the window moved by `offset`, kept inside the
/// window's frame clear of the band around it that holds its shadow: Compose's
/// `rememberPopupPositionProviderAtPosition(position, offset, windowMargin)`.
pub fn at_point(at: (f32, f32), offset: (f32, f32)) -> PositionAtPoint {
    let mut placed =
        popup_position_provider_at_position(Offset::new(at.0, at.1), crate::shadow_margin());
    placed.offset = Offset::new(offset.0, offset.1);
    placed
}

/// Emits `content` as a popup hanging `gap` logical pixels under the box it is declared in,
/// the control that opens it, their left edges together, or their right edges together and
/// above the control where the window has no room: Compose's `DropdownMenu`, placed by its
/// `DropdownMenuPositionProvider`, and dismissed as `dismissal` says. Declared in the control's
/// box, it takes no room there.
#[track_caller]
pub fn dropdown(gap: f32, dismissal: Dismissal, content: impl FnOnce()) {
    float(
        DropdownMenuPositionProvider {
            content_offset: Offset::new(0.0, gap),
            vertical_margin: 0.0,
        },
        dismissal,
        content,
    );
}

/// Emits `content` as a popup placed `offset` logical pixels from the top left of the box it
/// is declared in, over everything and kept inside the window, dismissed as `dismissal` says.
#[track_caller]
pub fn in_box_at(offset: (f32, f32), dismissal: Dismissal, content: impl FnOnce()) {
    float(
        rustle::AlignmentOffsetPositionProvider {
            alignment: rustle::BoxAlignment::TopStart,
            offset: Offset::new(offset.0, offset.1),
        },
        dismissal,
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

    /// A list hanging under a line goes over it when it would run past the bottom of the
    /// window, and a card over a line goes under it at the top of the window.
    #[test]
    fn a_popup_by_a_line_flips_to_the_side_with_room() {
        let window = Size::new(800.0, 600.0);
        let popup = Size::new(200.0, 100.0);
        let hung = |side, top| {
            ByLine {
                at: (50.0, top),
                height: 20.0,
                gap: 2.0,
                side,
            }
            .calculate_position(Rect::default(), window, popup)
        };
        assert_eq!(hung(Side::Below, 100.0), Offset::new(50.0, 122.0));
        assert_eq!(hung(Side::Below, 500.0), Offset::new(50.0, 398.0));
        assert_eq!(hung(Side::Above, 300.0), Offset::new(50.0, 198.0));
        assert_eq!(hung(Side::Above, 50.0), Offset::new(50.0, 72.0));
    }

    /// A menu declared in a control near the window's right edge lines up its right edge with
    /// the control's, and goes over the control near the bottom.
    #[test]
    fn a_dropdown_hangs_from_its_control() {
        let mut ui = rustle::Embedded::without_renderer(
            || {
                rustle::stack_with(Modifier::new().fill_max_size(), || {
                    rustle::stack_with(
                        Modifier::new().offset(700.0, 560.0).size(30.0, 20.0),
                        || {
                            dropdown(4.0, Dismissal::Never, || {
                                rustle::spacer(Modifier::new().size(200.0, 100.0).test_tag("menu"))
                            });
                        },
                    );
                });
            },
            || {},
        );
        ui.resize((800, 600), 1.0);
        ui.update();
        let menu = ui.node_with_tag("menu").expect("the menu is laid out");
        assert_eq!(menu.bounds.origin, Offset::new(530.0, 456.0));
    }

    /// The documentation beside a list starts just past the list's right edge, level with it.
    #[test]
    fn a_popup_beside_its_anchor_starts_past_its_right_edge() {
        let at = Beside { gap: 4.0 }.calculate_position(
            Rect::new(Offset::new(100.0, 80.0), Size::new(300.0, 120.0)),
            Size::new(1280.0, 800.0),
            Size::new(380.0, 160.0),
        );
        assert_eq!(at, Offset::new(404.0, 80.0));
    }
}

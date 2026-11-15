//! The part of the window floating surfaces, such as menus and tooltips, are
//! kept inside: the window provides it, and a surface that would run past
//! its right or bottom edge is moved back in or opened the other way.

use rustle::prelude::*;
use rustle::{CompositionLocal, Offset, Rect, Size};

/// The part of the window floating surfaces stay inside, in the window's
/// coordinates, when the window provided one.
static LOCAL_WINDOW_AREA: CompositionLocal<Option<Rect>> = CompositionLocal::new(|| None);

/// Emits `content` with `area`, in the window's coordinates, as the part of
/// the window the menus and tooltips in it are kept inside.
pub fn provide_window_area<R>(area: Rect, content: impl FnOnce() -> R) -> R {
    LOCAL_WINDOW_AREA.provide(Some(area), content)
}

/// The part of the window floating surfaces stay inside, when one was provided.
pub(crate) fn window_area() -> Option<Rect> {
    LOCAL_WINDOW_AREA.current()
}

/// Where a floating surface goes: moved by the offset across and down, and
/// opening upward from where it was emitted when the flag is set.
pub(crate) type Placement = ((f32, f32), bool);

/// Where a surface `size` big emitted at `origin` goes so it stays inside
/// `area`: moved by `offset` across and down and opening upward when
/// `above`, as asked, unless that runs past an edge, in which case it is
/// moved back in across and opened the other way, or moved up or down,
/// when there is no room the way it was asked.
pub(crate) fn fit(
    origin: Offset,
    size: Size,
    (across, down): (f32, f32),
    above: bool,
    area: Rect,
) -> Placement {
    let (left_edge, top_edge) = (area.origin.x, area.origin.y);
    let right_edge = left_edge + area.size.width;
    let bottom_edge = top_edge + area.size.height;
    let left = origin.x + across;
    let shifted = if left + size.width > right_edge {
        (right_edge - size.width).max(left_edge) - origin.x
    } else if left < left_edge {
        left_edge - origin.x
    } else {
        across
    };
    let below_fits = origin.y + down + size.height <= bottom_edge;
    let above_fits = origin.y - down - size.height >= top_edge;
    let upward = match (above, below_fits, above_fits) {
        (false, false, true) => true,
        (true, true, false) => false,
        (asked, _, _) => asked,
    };
    let lowered = if upward {
        let top = origin.y - down - size.height;
        if top < top_edge {
            down - (top_edge - top)
        } else {
            down
        }
    } else {
        let bottom = origin.y + down + size.height;
        if bottom > bottom_edge {
            (down - (bottom - bottom_edge)).max(top_edge - origin.y)
        } else {
            down
        }
    };
    ((shifted, lowered), upward)
}

/// Where a surface `size` big goes beside a control `anchor_height` tall
/// whose top-left corner is at `origin`: `gap` below it, or `gap` above it
/// when there is no room below and there is above, moved left or right to
/// stay inside `area`; as an offset from `origin`.
pub(crate) fn fit_beside(
    origin: Offset,
    anchor_height: f32,
    size: Size,
    gap: f32,
    area: Rect,
) -> (f32, f32) {
    let ((across, _), _) = fit(origin, size, (0.0, 0.0), false, area);
    let bottom_edge = area.origin.y + area.size.height;
    let below = anchor_height + gap;
    let up = -size.height - gap;
    let down = if origin.y + below + size.height > bottom_edge && origin.y + up >= area.origin.y {
        up
    } else {
        below
    };
    (across, down)
}

/// What a floating surface learned of itself last frame: where it was
/// emitted, in the window's coordinates, and how big it is.
pub(crate) struct Measures {
    /// Where it was emitted, once known.
    pub(crate) origin: Option<Offset>,
    /// How big it is, once known.
    pub(crate) size: Option<Size>,
    /// The modifier of a spacer of no size emitted where the surface starts, which learns where that is.
    pub(crate) marker: Modifier,
    /// The modifier of the surface, which learns how big it is.
    pub(crate) measured: Modifier,
}

/// Remembers where the floating surface emitted here starts and how big it is, for the next frame to place it by.
#[track_caller]
pub(crate) fn remember_measures() -> Measures {
    let origin = remember(|| None::<Offset>);
    let size = remember(|| None::<Size>);
    let (known_origin, known_size) = (origin.get(), size.get());
    let marker = Modifier::new()
        .size(0.0, 0.0)
        .on_globally_positioned(move |rect| {
            if origin.get() != Some(rect.origin) {
                origin.set(Some(rect.origin));
            }
        });
    let measured = Modifier::new().on_size_changed(move |now| {
        if size.get() != Some(now) {
            size.set(Some(now));
        }
    });
    Measures {
        origin: known_origin,
        size: known_size,
        marker,
        measured,
    }
}

/// Remembers where the floating surface emitted here starts and how big
/// it is, giving the place it is drawn at, moved by `offset` and opening
/// upward when `above` but kept inside the window area when one was
/// provided, and the modifiers that measure the origin and the surface for
/// the next frame.
#[track_caller]
pub(crate) fn remember_fit(offset: (f32, f32), above: bool) -> (Placement, Modifier, Modifier) {
    let measures = remember_measures();
    let placement = match (window_area(), measures.origin, measures.size) {
        (Some(area), Some(origin), Some(size)) => fit(origin, size, offset, above, area),
        _ => (offset, above),
    };
    (placement, measures.marker, measures.measured)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window 800 by 600 at its origin.
    fn area() -> Rect {
        Rect::new(Offset::new(0.0, 0.0), Size::new(800.0, 600.0))
    }

    /// A surface with room stays where it was asked to go.
    #[test]
    fn room_keeps_the_place_asked() {
        let placed = fit(
            Offset::new(100.0, 100.0),
            Size::new(200.0, 100.0),
            (0.0, 4.0),
            false,
            area(),
        );
        assert_eq!(placed, ((0.0, 4.0), false));
    }

    /// A surface at the right edge moves left, and one at the bottom opens upward.
    #[test]
    fn edges_push_back_and_flip() {
        let placed = fit(
            Offset::new(700.0, 550.0),
            Size::new(200.0, 100.0),
            (0.0, 4.0),
            false,
            area(),
        );
        assert_eq!(placed, ((-100.0, 4.0), true));
    }
}

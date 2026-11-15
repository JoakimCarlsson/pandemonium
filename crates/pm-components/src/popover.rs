//! A raised surface floating above everything else where it is emitted,
//! such as the menus and the branch list of the Git panel; a click outside
//! it or Escape dismisses it.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, Modifiers, Rect};

use crate::size::Size;
use rustle::color_scheme;

/// How a popover is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverVariant {
    /// A raised surface with a hairline border.
    #[default]
    Raised,
    /// The window background with a hairline border, for a popover over a raised surface.
    Flat,
}

/// A popover with a variant and a size.
#[derive(Clone)]
pub struct Popover {
    /// How it is drawn.
    variant: PopoverVariant,
    /// How much padding it has inside.
    size: Size,
    /// How wide it is.
    width: f32,
    /// How far it is moved from where it is emitted, across and down.
    offset: (f32, f32),
    /// What a click outside it does.
    on_dismiss: Option<Rc<dyn Fn()>>,
    /// Whether it opens upward from where it is emitted, for a field near the bottom of the window.
    above: bool,
}

impl Popover {
    /// A raised popover 220 wide, 4 below where it is emitted.
    pub fn new() -> Self {
        Self {
            variant: PopoverVariant::Raised,
            size: Size::Sm,
            width: 220.0,
            offset: (0.0, 4.0),
            on_dismiss: None,
            above: false,
        }
    }

    /// The popover opening upward from where it is emitted when `above`.
    pub fn above(mut self, above: bool) -> Self {
        self.above = above;
        self
    }

    /// The popover in `variant`.
    pub fn variant(mut self, variant: PopoverVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The popover with padding for `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The popover `width` wide.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The popover moved `across` and `down` from where it is emitted.
    pub fn offset(mut self, across: f32, down: f32) -> Self {
        self.offset = (across, down);
        self
    }

    /// The popover calling `on_dismiss` when clicked outside or on Escape.
    pub fn on_dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(on_dismiss));
        self
    }

    /// Emits the popover holding `content`.
    #[track_caller]
    pub fn show(self, content: impl FnOnce()) {
        let scheme = color_scheme();
        let padding = match self.size {
            Size::Xs => 2.0,
            Size::Sm => 4.0,
            Size::Md => 6.0,
            Size::Lg => 8.0,
        };
        let fill = match self.variant {
            PopoverVariant::Raised => scheme.surface_container,
            PopoverVariant::Flat => scheme.background,
        };
        let dismiss = self.on_dismiss.clone();
        let escape = self.on_dismiss.clone();
        let shift = remember(|| (0.0_f32, 0.0_f32));
        let (shift_x, shift_y) = shift.get();
        let fitting = shift.clone();
        catching_outside(
            self.above,
            move || {
                if let Some(dismiss) = &dismiss {
                    dismiss();
                }
            },
            || {
                column_with(
                    Modifier::new()
                        .width(self.width)
                        .offset(
                            self.offset.0 + shift_x,
                            shift_y
                                + if self.above {
                                    -self.offset.1
                                } else {
                                    self.offset.1
                                },
                        )
                        .on_placed(move |rect, window| {
                            let wanted = kept_inside(rect, window, fitting.get());
                            if wanted != fitting.get() {
                                fitting.set(wanted);
                            }
                        })
                        .shadow(8.0, 8.0)
                        .background_rounded(fill, 8.0)
                        .border(1.0, scheme.outline_variant, 8.0)
                        .clickable_plain(|| {})
                        .on_key_event(move |event| match &escape {
                            Some(escape) if event.is(Key::Escape, Modifiers::NONE) => {
                                escape();
                                true
                            }
                            _ => false,
                        })
                        .padding(padding),
                    content,
                );
            },
        );
    }
}

/// How far a popover laid out at `rect`, already moved by `shift`, must be moved from where it was asked to be so it stays inside a window of `window`'s size, keeping a small margin.
fn kept_inside(rect: Rect, window: rustle::Size, shift: (f32, f32)) -> (f32, f32) {
    let margin = 4.0;
    let fit = |start: f32, length: f32, room: f32, moved: f32| {
        let asked = start - moved;
        let most = (room - margin - length).max(margin);
        let placed = asked.clamp(margin.min(most), most);
        (placed - asked).round()
    };
    (
        fit(rect.origin.x, rect.size.width, window.width, shift.0),
        fit(rect.origin.y, rect.size.height, window.height, shift.1),
    )
}

/// Emits `content` in the layer above everything else, opening upward when
/// `above`, over an invisible layer larger than any window whose click calls
/// `on_dismiss`.
#[track_caller]
fn catching_outside(above: bool, on_dismiss: impl Fn() + 'static, content: impl FnOnce()) {
    let layered = move || {
        spacer(
            Modifier::new()
                .size(20_000.0, 20_000.0)
                .offset(-10_000.0, if above { 10_000.0 } else { -10_000.0 })
                .clickable_plain(on_dismiss),
        );
        content();
    };
    if above {
        floating_above(Modifier::new(), layered);
    } else {
        floating(Modifier::new(), layered);
    }
}

impl Default for Popover {
    /// The same as [`Popover::new`].
    fn default() -> Self {
        Self::new()
    }
}

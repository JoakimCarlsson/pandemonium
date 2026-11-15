//! A raised surface floating above everything else where it is emitted,
//! such as the menus and the branch list of the Git panel; a click outside
//! it or Escape dismisses it.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, Modifiers};

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
        let (((across, down), above), marker, measured) =
            crate::window_area::remember_fit(self.offset, self.above);
        catching_outside(
            above,
            move || {
                if let Some(dismiss) = &dismiss {
                    dismiss();
                }
            },
            || {
                spacer(marker);
                column_with(
                    measured
                        .width(self.width)
                        .offset(across, if above { -down } else { down })
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

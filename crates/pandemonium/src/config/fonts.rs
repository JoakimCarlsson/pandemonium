//! The faces and sizes the window's text is set in.
//!
//! The theme brings a type scale; the reader's fonts are laid over it every
//! frame, so a family's own scale and the reader's sizes never have to be
//! reconciled in a theme file.

use pm_gfx::FontStyle;
use pm_ui::TextScale;

/// The body size the built-in scale is drawn at, which the reader's own
/// interface size scales every step from.
const SCALE_BASE: f32 = 14.0;

/// Which of the two families a font preference is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontSlot {
    /// The family prose and labels are set in.
    Interface,
    /// The monospaced family code, paths and terminals are set in.
    Buffer,
}

impl FontSlot {
    /// What the font picker says while nothing has been typed into it.
    pub const fn placeholder(self) -> &'static str {
        match self {
            Self::Interface => "Set the interface in…",
            Self::Buffer => "Set code in…",
        }
    }
}

/// The faces and sizes the window's text is set in.
#[derive(Clone, Debug, PartialEq)]
pub struct Fonts {
    /// The family prose and labels are set in, or none for the editor's pick.
    pub interface_family: Option<String>,
    /// The body size of the interface, every other step scaled with it.
    pub interface_size: f32,
    /// The family code is set in, or none for the editor's pick.
    pub buffer_family: Option<String>,
    /// The size code is set in.
    pub buffer_size: f32,
    /// The weight code is set in, on the usual 100..=900 scale.
    pub buffer_weight: u16,
    /// The distance between two lines of code, as a multiple of its size.
    pub buffer_line_height: f32,
    /// The size a terminal grid is set in.
    pub terminal_size: f32,
}

impl Default for Fonts {
    /// The sizes the built-in scale is drawn at, in the editor's own picks.
    fn default() -> Self {
        Self {
            interface_family: None,
            interface_size: SCALE_BASE,
            buffer_family: None,
            buffer_size: 14.0,
            buffer_weight: 400,
            buffer_line_height: 1.5,
            terminal_size: 14.0,
        }
    }
}

impl Fonts {
    /// The family `slot` names, or none for the editor's pick.
    pub fn family(&self, slot: FontSlot) -> Option<&str> {
        match slot {
            FontSlot::Interface => self.interface_family.as_deref(),
            FontSlot::Buffer => self.buffer_family.as_deref(),
        }
    }

    /// Sets the family `slot` names, none being the editor's pick.
    pub fn set_family(&mut self, slot: FontSlot, family: Option<String>) {
        match slot {
            FontSlot::Interface => self.interface_family = family,
            FontSlot::Buffer => self.buffer_family = family,
        }
    }

    /// `scale` at the reader's sizes.
    pub fn scale(&self, scale: TextScale) -> TextScale {
        let factor = self.interface_size / SCALE_BASE;
        let scaled = |style: FontStyle| FontStyle {
            size: (style.size * factor).round(),
            line_height: (style.line_height * factor).round(),
            ..style
        };
        TextScale {
            xs: scaled(scale.xs),
            sm: scaled(scale.sm),
            base: scaled(scale.base),
            lg: scaled(scale.lg),
            xl: scaled(scale.xl),
            xxl: scaled(scale.xxl),
            code: FontStyle::new(self.buffer_size)
                .mono()
                .weight(self.buffer_weight)
                .line_height((self.buffer_size * self.buffer_line_height).round()),
            terminal: FontStyle::new(self.terminal_size).mono(),
        }
    }
}

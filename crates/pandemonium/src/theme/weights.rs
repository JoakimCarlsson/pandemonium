//! Every emphasis of a theme, by the name a theme file calls it.
//!
//! An emphasis is how strongly a translucent part of the window is drawn,
//! and a theme is as free to set one as it is to set a colour: a light
//! theme's selection wash has to be fainter than a dark theme's to read.

use pm_ui::Theme;

/// One emphasis of a theme, and how to write it.
#[derive(Clone, Copy, Debug)]
pub struct Weight {
    /// Its name, as a theme file writes it.
    key: &'static str,
    /// Writes it into a theme.
    write: fn(&mut Theme, f32),
}

impl Weight {
    /// Sets this emphasis of `theme` to `value`.
    pub fn write(&self, theme: &mut Theme, value: f32) {
        (self.write)(theme, value);
    }
}

/// Declares [`WEIGHTS`] from one key per emphasis.
macro_rules! weights {
    ($($key:ident),* $(,)?) => {
        /// Every emphasis a theme names.
        pub const WEIGHTS: &[Weight] = &[$(Weight {
            key: stringify!($key),
            write: |theme, value| theme.emphasis.$key = value,
        }),*];
    };
}

weights! {
    selection,
    current_line,
    scrollbar,
    scrollbar_active,
    halo,
    dim,
    hover_lift,
    occurrence,
    guide,
    search,
    search_current,
    bracket,
    change,
}

/// The emphasis `key` names.
pub fn weight(key: &str) -> Option<&'static Weight> {
    WEIGHTS.iter().find(|weight| weight.key == key)
}

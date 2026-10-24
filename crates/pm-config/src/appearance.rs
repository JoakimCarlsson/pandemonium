//! How the window looks: its colours, its fonts and how closely it sets things.

use serde::{Deserialize, Serialize};

/// Whether the window is light or dark.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// As the desktop says.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// How closely rows, padding and gaps are set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// The room the window was drawn for.
    #[default]
    Comfortable,
    /// Less room around everything, so more fits.
    Compact,
}

impl Density {
    /// How much of the comfortable room each step of spacing takes.
    pub fn scale(self) -> f32 {
        match self {
            Self::Comfortable => 1.0,
            Self::Compact => 0.8,
        }
    }
}

/// The size the interface was drawn for, in logical pixels, which its own size is measured against.
pub const INTERFACE_BASE: u32 = 13;

/// How the window looks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    /// Whether the window is light or dark.
    pub theme_mode: ThemeMode,
    /// The theme drawn while it is light, by name.
    pub light_theme: String,
    /// The theme drawn while it is dark, by name.
    pub dark_theme: String,
    /// The font the interface is set in, by family name; empty for the one the window ships with.
    pub interface_font: String,
    /// How big the interface's text is, in logical pixels.
    pub interface_size: u32,
    /// The font code is set in, by family name; empty for the one the window ships with.
    pub code_font: String,
    /// How closely things are set.
    pub density: Density,
}

impl Default for AppearanceSettings {
    /// The window as it comes: following the desktop, in the default theme and fonts.
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            light_theme: "Default".to_owned(),
            dark_theme: "Default".to_owned(),
            interface_font: String::new(),
            interface_size: INTERFACE_BASE,
            code_font: String::new(),
            density: Density::Comfortable,
        }
    }
}

impl AppearanceSettings {
    /// How much bigger than drawn for the interface's text is.
    pub fn text_scale(&self) -> f32 {
        self.interface_size as f32 / INTERFACE_BASE as f32
    }
}

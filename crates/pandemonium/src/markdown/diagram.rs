//! Mermaid diagrams rendered in the active theme's colours.

use mermaid_svg::{Theme as MermaidTheme, render_with};
use pm_gfx::{Image, Rgba};
use pm_ui::{Appearance, Theme};

/// Colours that determine a diagram's rasterized appearance.
#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct Palette {
    /// The diagram canvas.
    background: String,
    /// Primary labels and edges.
    text: String,
    /// Secondary labels.
    muted: String,
    /// Nodes and participant headers.
    fill: String,
    /// Node outlines.
    border: String,
    /// Emphasized edges and outlines.
    accent: String,
}

impl Palette {
    /// Resolves the colours used by Mermaid from the active editor theme.
    pub(super) fn from_theme(theme: &Theme) -> Self {
        Self {
            background: hex(theme.colors.surface),
            text: hex(theme.colors.text),
            muted: hex(theme.colors.text_muted),
            fill: hex(theme.colors.surface_selected),
            border: hex(theme.colors.border),
            accent: hex(theme.colors.link),
        }
    }

    /// Renders `source` as a full-colour image at the window's scale.
    pub(super) fn render(&self, source: &str, scale: f32, appearance: Appearance) -> Option<Image> {
        let mut theme = match appearance {
            Appearance::Dark => MermaidTheme::dark(),
            Appearance::Light => MermaidTheme::default_theme(),
        };
        theme.bg = self.background.clone().into();
        theme.fg = self.text.clone().into();
        theme.fg_muted = self.muted.clone().into();
        theme.actor_fill = self.fill.clone().into();
        theme.actor_stroke = self.border.clone().into();
        theme.lifeline = self.border.clone().into();
        theme.arrow_stroke = self.text.clone().into();
        theme.note_fill = self.fill.clone().into();
        theme.note_stroke = self.border.clone().into();
        theme.activation_fill = self.fill.clone().into();
        theme.activation_stroke = self.border.clone().into();
        theme.frame_label_fill = self.fill.clone().into();
        theme.flow_node_fill = self.fill.clone().into();
        theme.flow_node_stroke = self.accent.clone().into();
        theme.flow_edge_stroke = self.text.clone().into();
        theme.flow_label_bg = self.background.clone().into();
        theme.flow_cluster_fill = self.background.clone().into();
        theme.flow_cluster_stroke = self.border.clone().into();
        let svg = render_with(source, &theme).ok()?;
        Image::from_svg(&svg, scale)
    }
}

/// Formats an sRGB colour for the SVG renderer.
fn hex(color: Rgba) -> String {
    let byte = |channel: f32| (channel * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        byte(color.r),
        byte(color.g),
        byte(color.b)
    )
}

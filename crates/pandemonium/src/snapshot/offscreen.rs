//! Drawing a window's content once, offscreen, into a PNG file, through
//! rustle's [`preview_pixels`](rustle::preview_pixels): at a size and a scale,
//! at a fixed time on the frame's clock, with the pointer resting at a point
//! (so hover styles and tooltips show), a control holding focus (so its ring
//! and caret show), in the light or dark theme of a family. Edges are rounded
//! to whole logical pixels.

use std::time::Duration;

use pm_components::kit::frame::{self, Frame};
use pm_components::kit::icon;
use pm_components::kit::overlay;
use pm_components::kit::report::{self, Report};
use pm_components::kit::theme::{self, Appearance, ThemeFamily, Tokens};
use pm_components::kit::{FocusId, TextSelection};
use rustle::{Offset, PixelRounding, PreviewOptions};

/// How to draw one snapshot.
#[derive(Clone, Debug)]
pub(crate) struct Shot {
    /// Width in logical pixels.
    pub(crate) width: f32,
    /// Height in logical pixels.
    pub(crate) height: f32,
    /// Physical pixels per logical pixel.
    pub(crate) scale: f32,
    /// The time on the frame's clock, in milliseconds since the app started.
    pub(crate) clock_ms: f32,
    /// Where the pointer rests, in logical pixels, if over the window.
    pub(crate) pointer: Option<(f32, f32)>,
    /// The control holding keyboard focus.
    pub(crate) focused: Option<FocusId>,
    /// The stretch of selectable text selected.
    pub(crate) selection: Option<TextSelection>,
    /// Whether what is not drawn stays see-through, as around a floating window's frame.
    pub(crate) transparent: bool,
    /// The theme family.
    pub(crate) family: ThemeFamily,
    /// Light or dark.
    pub(crate) appearance: Appearance,
}

impl Shot {
    /// A snapshot `width` by `height` logical pixels at scale one, 700 ms in,
    /// with no pointer and no focus, in the dark theme of the default family.
    pub(crate) fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            scale: 1.0,
            clock_ms: 700.0,
            pointer: None,
            focused: None,
            selection: None,
            transparent: false,
            family: theme::default_family(),
            appearance: Appearance::Dark,
        }
    }
}

/// Draws `content` as `shot` says into a PNG file at `path`, inside the
/// theme of `shot` and a root that starts the frame's report and draws
/// tooltips over everything, and returns what was painted where.
///
/// # Errors
///
/// Returns an error when no GPU adapter can be opened, the picture cannot be
/// read back or encoded, or the file cannot be written.
pub(crate) fn write_png(
    path: &std::path::Path,
    shot: &Shot,
    content: impl Fn() + 'static,
) -> Result<Report, Box<dyn std::error::Error>> {
    frame::set(Frame {
        focused: shot.focused,
        clock_ms: shot.clock_ms,
        selection: shot.selection,
        ..Frame::default()
    });
    rustle_wgpu::set_legible_text(false);
    rustle_wgpu::set_gamma_blending(true);
    icon::set_pixel_scale(shot.scale);
    overlay::set_tooltip_delay(0.0);
    overlay::reset_tooltip();
    let tokens = Tokens::new(&shot.family, shot.appearance);
    let root = move || {
        theme::provide(tokens.clone(), || {
            report::root(|| {
                content();
                overlay::tooltips(report::MAIN);
            })
        })
    };
    let options = PreviewOptions {
        size: (shot.width, shot.height),
        scale: shot.scale,
        dark: shot.appearance == Appearance::Dark,
        clock: Duration::from_secs_f32(shot.clock_ms.max(0.0) / 1000.0),
        pointer: shot.pointer.map(|(x, y)| Offset::new(x, y)),
        transparent: shot.transparent,
        rounding: PixelRounding::Logical,
    };
    let (width, height, rgba) = rustle::preview_pixels(root, options)?;
    let image = rustle::ImageData::from_rgba(width, height, rgba)
        .ok_or("the picture's size does not match its pixels")?;
    let png = image.encode_png()?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, png)?;
    Ok(report::last())
}

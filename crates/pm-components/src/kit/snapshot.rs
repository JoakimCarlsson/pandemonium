//! Drawing a window's content once, offscreen, into pixels or a PNG file.
//!
//! The host's `--snapshot` and the kit gallery draw with this: a root
//! composable laid out at a size and a scale, at a fixed time on the
//! frame's clock, with the pointer resting at a point (so hover styles and
//! tooltips show), a control holding focus (so its ring and caret show), in
//! the light or dark theme of a family. It composes twice, as the old
//! two-pass snapshot built twice, so what depends on the frame before
//! (scrollbars that show once a box was found to scroll, tooltips) is drawn.
//! Edges are rounded to whole logical pixels, as the old layout rounded
//! them. It needs a GPU adapter but no display. It belongs to the host; it lives
//! in the kit so the gallery and the host share one implementation.

use std::time::Duration;

use rustle::{Embedded, Offset, PixelRounding};

use crate::kit::focus::FocusId;
use crate::kit::frame::{self, Frame};
use crate::kit::icon;
use crate::kit::overlay;
use crate::kit::report::{self, Report};
use crate::kit::select::TextSelection;
use crate::kit::theme::{self, Appearance, ThemeFamily, Tokens};

/// How to draw one snapshot.
#[derive(Clone, Debug)]
pub struct Shot {
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
    /// Physical pixels per logical pixel.
    pub scale: f32,
    /// The time on the frame's clock, in milliseconds since the app started.
    pub clock_ms: f32,
    /// Where the pointer rests, in logical pixels, if over the window.
    pub pointer: Option<(f32, f32)>,
    /// The control holding keyboard focus.
    pub focused: Option<FocusId>,
    /// The stretch of selectable text selected.
    pub selection: Option<TextSelection>,
    /// Whether what is not drawn stays see-through, as around a floating window's frame.
    pub transparent: bool,
    /// The theme family.
    pub family: ThemeFamily,
    /// Light or dark.
    pub appearance: Appearance,
}

impl Shot {
    /// A snapshot `width` by `height` logical pixels at scale one, 700 ms in,
    /// with no pointer and no focus, in the dark theme of the default family.
    pub fn new(width: f32, height: f32) -> Self {
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

/// A picture drawn by [`render`]: its size in physical pixels, its straight
/// RGBA pixels row by row from the top, and the report of what was painted.
#[derive(Clone, Debug)]
pub struct Pixels {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Straight RGBA bytes, row by row from the top.
    pub rgba: Vec<u8>,
    /// What was painted where.
    pub report: Report,
}

/// Draws `content` as `shot` says and returns its pixels.
///
/// `content` is composed inside the theme of `shot`, inside a root that
/// starts the frame's report and draws tooltips over everything.
///
/// # Errors
///
/// Returns an error when no GPU adapter can be opened or the picture cannot be read back.
pub fn render(
    shot: &Shot,
    content: impl Fn() + 'static,
) -> Result<Pixels, Box<dyn std::error::Error>> {
    let physical = (
        (shot.width * shot.scale).round().max(1.0) as u32,
        (shot.height * shot.scale).round().max(1.0) as u32,
    );
    frame::set(Frame {
        focused: shot.focused,
        clock_ms: shot.clock_ms,
        selection: shot.selection,
        ..Frame::default()
    });
    rustle_wgpu::set_legible_text(false);
    icon::set_pixel_scale(shot.scale);
    overlay::set_tooltip_delay(0.0);
    overlay::reset_tooltip();
    let tokens = Tokens::new(&shot.family, shot.appearance);
    rustle_wgpu::set_gamma_blending(true);
    let mut renderer = rustle_wgpu::Renderer::offscreen(physical.0, physical.1)?;
    renderer.set_transparent(shot.transparent);
    let root = move || {
        theme::provide(tokens.clone(), || {
            report::root(|| {
                content();
                overlay::tooltips();
            })
        })
    };
    let mut ui = Embedded::new(root, renderer, || {});
    ui.set_pixel_rounding(PixelRounding::Logical);
    ui.set_dark(shot.appearance == Appearance::Dark);
    let clock = Duration::from_secs_f32(shot.clock_ms.max(0.0) / 1000.0);
    ui.set_clock(move || clock);
    ui.resize(physical, shot.scale);
    ui.update();
    if let Some((x, y)) = shot.pointer {
        ui.pointer_moved(Offset::new(x, y));
    }
    ui.invalidate();
    ui.update();
    ui.invalidate();
    ui.update();
    let (width, height, rgba) = ui.snapshot()?;
    Ok(Pixels {
        width,
        height,
        rgba,
        report: report::last(),
    })
}

/// Draws `content` as `shot` says into a PNG file at `path`.
///
/// # Errors
///
/// Returns an error when drawing fails, the picture cannot be encoded, or the file cannot be written.
pub fn write_png(
    path: &std::path::Path,
    shot: &Shot,
    content: impl Fn() + 'static,
) -> Result<Report, Box<dyn std::error::Error>> {
    let pixels = render(shot, content)?;
    let image = rustle::ImageData::from_rgba(pixels.width, pixels.height, pixels.rgba)
        .ok_or("the picture's size does not match its pixels")?;
    let png = image.encode_png()?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, png)?;
    Ok(pixels.report)
}

/// Lays `content` out at `width` by `height` logical pixels without drawing
/// it, with `focused` holding focus, and returns what was painted where.
///
/// For tests that asked the old frame output which controls were drawn and
/// where: no GPU is needed, text is measured with the fonts of this thread.
pub fn lay_out(
    width: f32,
    height: f32,
    focused: Option<FocusId>,
    content: impl Fn() + 'static,
) -> Report {
    frame::set(Frame {
        focused,
        ..Frame::default()
    });
    rustle_wgpu::set_legible_text(false);
    let tokens = Tokens::default();
    let root = move || theme::provide(tokens.clone(), || report::root(&content));
    let mut ui = Embedded::without_renderer(root, || {});
    ui.set_pixel_rounding(PixelRounding::Logical);
    ui.resize((width.round() as u32, height.round() as u32), 1.0);
    ui.update();
    ui.invalidate();
    ui.update();
    report::last()
}

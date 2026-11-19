//! Icons: rustle's Phosphor drawings ([`Icon`], such as `phosphor::X` or
//! `phosphor::fill::X` for a selected state), tinted with a tone like text,
//! and artwork that arrives while the app runs, rasterised into a coverage
//! mask.
//!
//! The panel drawings Phosphor lacks are made from its own:
//! [`panel_right`] and [`panel_right_filled`] are its left sidebar drawn
//! mirrored, [`panel_bottom`] its browser window drawn upside down. Only artwork that arrives at run time, such as an
//! extension's icon or a file icon ([`Artwork::arrived`]), goes through
//! resvg. The spinner and the turning icon turn on rustle's infinite
//! animation, on the composition's frame clock.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

use rustle::{Icon, ImageData, Modifier, Path, PathCommand, phosphor};

use crate::theme::{self, Tone, sp};

/// A window with a panel along its right side: Phosphor's regular sidebar
/// drawn mirrored.
pub fn panel_right() -> Icon {
    MIRRORED_SIDEBAR.clone()
}

/// A window with its right panel filled to show it is open: Phosphor's
/// filled sidebar drawn mirrored.
pub fn panel_right_filled() -> Icon {
    MIRRORED_SIDEBAR_FILLED.clone()
}

/// A window with a panel along its bottom: Phosphor's browser window drawn
/// upside down.
pub fn panel_bottom() -> Icon {
    UPENDED_BROWSER.clone()
}

/// Phosphor's regular sidebar mirrored left to right, made once.
static MIRRORED_SIDEBAR: LazyLock<Path> = LazyLock::new(|| flipped(&phosphor::SIDEBAR_SIMPLE));

/// Phosphor's browser window turned upside down, made once: a window with a
/// band along its bottom, the panel at the bottom of the screen.
static UPENDED_BROWSER: LazyLock<Path> = LazyLock::new(|| upended(&phosphor::BROWSER));

/// Phosphor's filled sidebar mirrored left to right, made once.
static MIRRORED_SIDEBAR_FILLED: LazyLock<Path> =
    LazyLock::new(|| flipped(&phosphor::fill::SIDEBAR_SIMPLE));

/// `path` mirrored left to right, kept as constant data so renderers cache it.
fn flipped(path: &Path) -> Path {
    let (width, _) = path.size();
    transformed(path, |x, y| (width - x, y))
}

/// `path` mirrored top to bottom, kept as constant data so renderers cache it.
fn upended(path: &Path) -> Path {
    let (_, height) = path.size();
    transformed(path, |x, y| (x, height - y))
}

/// `path` with every point moved by `point`, kept as constant data so renderers cache it.
fn transformed(path: &Path, point: impl Fn(f32, f32) -> (f32, f32)) -> Path {
    let (width, height) = path.size();
    let commands: Vec<PathCommand> = path
        .commands()
        .iter()
        .map(|command| match *command {
            PathCommand::MoveTo(x, y) => {
                let (x, y) = point(x, y);
                PathCommand::MoveTo(x, y)
            }
            PathCommand::LineTo(x, y) => {
                let (x, y) = point(x, y);
                PathCommand::LineTo(x, y)
            }
            PathCommand::QuadTo(cx, cy, x, y) => {
                let ((cx, cy), (x, y)) = (point(cx, cy), point(x, y));
                PathCommand::QuadTo(cx, cy, x, y)
            }
            PathCommand::CubicTo(ax, ay, bx, by, x, y) => {
                let ((ax, ay), (bx, by), (x, y)) = (point(ax, ay), point(bx, by), point(x, y));
                PathCommand::CubicTo(ax, ay, bx, by, x, y)
            }
            PathCommand::Close => PathCommand::Close,
        })
        .collect();
    Path::from_static(
        Box::leak(commands.into_boxed_slice()),
        width,
        height,
        path.fill_rule(),
    )
}

/// A piece of vector artwork, named so its rasterised masks can be kept.
///
/// The name is the identity: two pieces with the same name are the same
/// artwork, and the source is only read the first time one is rasterised.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Artwork {
    /// What it is called, unique across the artwork the app draws.
    pub name: &'static str,
    /// The SVG document it is drawn from.
    pub source: &'static str,
}

/// The artwork that arrived while the app runs, by name, each kept for as long as the app runs.
static ARRIVED: LazyLock<Mutex<HashMap<String, Artwork>>> = LazyLock::new(Default::default);

thread_local! {
    /// Masks rasterised so far, by artwork name and size in physical pixels.
    static MASKS: RefCell<HashMap<(&'static str, u32), Option<ImageData>>> = RefCell::new(HashMap::new());
    /// Physical pixels per logical pixel of the window being drawn, which masks are rasterised at.
    static PIXEL_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

impl Artwork {
    /// The artwork `source` that arrived while the app runs, such as the icon
    /// of an extension, called `name`; artwork of one name is kept once, the
    /// first source given for it standing for every later one.
    pub fn arrived(name: &str, source: &str) -> Self {
        let mut held = ARRIVED.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(artwork) = held.get(name) {
            return *artwork;
        }
        let artwork = Self {
            name: Box::leak(name.to_owned().into_boxed_str()),
            source: Box::leak(source.to_owned().into_boxed_str()),
        };
        held.insert(name.to_owned(), artwork);
        artwork
    }

    /// The artwork's coverage in a square `pixels` physical pixels on a side,
    /// kept once made, or nothing when the size is zero or the source cannot be drawn.
    pub fn mask(&self, pixels: u32) -> Option<ImageData> {
        MASKS.with(|masks| {
            masks
                .borrow_mut()
                .entry((self.name, pixels))
                .or_insert_with(|| rasterise(self.source, pixels))
                .clone()
        })
    }
}

/// `source` drawn into a square `pixels` on a side, as coverage, or nothing when it cannot be drawn.
fn rasterise(source: &str, pixels: u32) -> Option<ImageData> {
    let tree = resvg::usvg::Tree::from_str(source, &resvg::usvg::Options::default()).ok()?;
    let mut canvas = resvg::tiny_skia::Pixmap::new(pixels, pixels)?;
    let scale = pixels as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut canvas.as_mut(),
    );
    let coverage: Vec<u8> = canvas.pixels().iter().map(|pixel| pixel.alpha()).collect();
    ImageData::from_alpha(pixels, pixels, &coverage)
}

/// Rasterises artwork for a window drawn at `scale` physical pixels per logical pixel, so masks stay sharp.
pub fn set_pixel_scale(scale: f32) {
    PIXEL_SCALE.with(|held| held.set(scale.max(0.5)));
}

/// Emits `icon` in a square `steps` of the spacing scale on a side, tinted
/// with `tone`: `glyph(icon, steps, tone)`, or a box sized `size(steps)` with `.icon(icon)`.
#[track_caller]
pub fn icon(icon: Icon, steps: f32, tone: impl Into<Tone>) {
    icon_with(Modifier::new(), icon, sp(steps), tone);
}

/// Emits `icon` in a square `pixels` logical pixels on a side, tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn icon_with(modifier: Modifier, icon: Icon, pixels: f32, tone: impl Into<Tone>) {
    path_with(modifier, icon, pixels, tone);
}

/// Emits the vector outline `path`, such as `phosphor::X`, in a square
/// `pixels` logical pixels on a side, tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn path_with(modifier: Modifier, path: Path, pixels: f32, tone: impl Into<Tone>) {
    let color = theme::color(tone);
    rustle::canvas(modifier.size(pixels, pixels), move |canvas| {
        let side = canvas.width().min(canvas.height());
        let (x, y) = (
            (canvas.width() - side) / 2.0,
            (canvas.height() - side) / 2.0,
        );
        canvas.path(&path, x, y, side, side, color);
    });
}

/// Emits `art` in a square `pixels` logical pixels on a side, tinted with
/// `tone`, with `modifier` around it: `.artwork(svg)`.
#[track_caller]
pub fn artwork_with(modifier: Modifier, art: Artwork, pixels: f32, tone: impl Into<Tone>) {
    let color = theme::color(tone);
    let scale = PIXEL_SCALE.with(std::cell::Cell::get);
    let physical = (pixels * scale).round().max(1.0) as u32;
    match art.mask(physical) {
        Some(mask) => rustle::mask_image(modifier.size(pixels, pixels), &mask, color),
        None => rustle::spacer(modifier.size(pixels, pixels)),
    }
}

/// How long a spinner takes to turn once.
const TURN: std::time::Duration = std::time::Duration::from_secs(1);

/// The angle, in degrees, of something turning once a [`TURN`], on the
/// composition's frame clock.
#[track_caller]
fn turning() -> rustle::State<f32> {
    rustle::infinite_float_repeating(0.0, 360.0, TURN, rustle::Easing::Linear)
}

/// Emits a spinner `steps` of the spacing scale across in `tone`: rustle's
/// dashed ring, the loading icon, turning once a second, its angle read when
/// it is painted, so it turns without composing again.
#[track_caller]
pub fn spinner(steps: f32, tone: impl Into<Tone>) {
    let turned = turning();
    rustle::dashed_spinner(sp(steps), theme::color(tone), move || turned.get());
}

/// Emits `icon` turning once a second, `steps` of the spacing scale across,
/// in `tone`, for an icon other than the loading ring, which is [`spinner`]:
/// its angle read when it is painted, so it turns without composing again.
#[track_caller]
pub fn spinning_icon(icon: Icon, steps: f32, tone: impl Into<Tone>) {
    let turned = turning();
    icon_with(
        Modifier::new().rotate_with(move || turned.get()),
        icon,
        sp(steps),
        tone,
    );
}

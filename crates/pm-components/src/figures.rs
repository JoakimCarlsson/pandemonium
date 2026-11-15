//! The pictures a message shows: diagrams written as fenced mermaid code,
//! formulas written in TeX and images it names, each drawn into pixels once
//! and kept under a key, then shown at their own size.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use base64::Engine;
use rustle::prelude::*;

use crate::app_theme::{ExtendedColors, extended_colors};
use crate::card::{Card, CardVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::{ColorScheme, color_scheme};

/// How many pixels a vector picture is drawn with per logical pixel, so it
/// stays sharp on dense screens.
const DENSITY: f32 = 2.0;

/// The longest side a picture is drawn at, in pixels, so its texture fits any GPU.
const LONGEST_SIDE: f32 = 4096.0;

/// How tall the type of a formula is, in points, as a ratio.
const FORMULA_POINTS: (i64, i64) = (27, 2);

/// The fonts of this machine, loaded once, so the text in a picture is drawn.
static FONTS: LazyLock<Arc<resvg::usvg::fontdb::Database>> = LazyLock::new(|| {
    let mut fonts = resvg::usvg::fontdb::Database::new();
    fonts.load_system_fonts();
    let installed = |wanted: &str| {
        fonts
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == wanted))
    };
    let sans = SANS_SERIF.into_iter().find(|family| installed(family));
    let mono = MONOSPACE.into_iter().find(|family| installed(family));
    if let Some(family) = sans {
        fonts.set_sans_serif_family(family);
    }
    if let Some(family) = mono {
        fonts.set_monospace_family(family);
    }
    Arc::new(fonts)
});

/// The families tried, first found first, for text a picture sets in a sans-serif face.
const SANS_SERIF: [&str; 7] = [
    "Inter",
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Cantarell",
    "Helvetica",
    "Arial",
];

/// The families tried, first found first, for text a picture sets in a monospaced face.
const MONOSPACE: [&str; 5] = [
    "JetBrains Mono",
    "Noto Sans Mono",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Menlo",
];

thread_local! {
    /// The figures drawn so far, by key, each its drawing or why there is none.
    static DRAWN: RefCell<HashMap<u64, Result<Rc<Drawing>, String>>> = RefCell::default();
}

/// What a figure is drawn from.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FigureSource {
    /// A diagram written in mermaid.
    Diagram(String),
    /// A formula written in TeX.
    Formula(String),
    /// An image by its address, with the words that stand for it.
    Image {
        /// Where the picture is: a `data:` address, a web address or a path.
        source: String,
        /// What stands for the picture where it cannot be shown.
        alt: String,
    },
}

/// Pixels a figure was drawn into and the logical size it shows at.
#[derive(Clone, Debug, PartialEq)]
struct Drawing {
    /// The pixels.
    image: ImageData,
    /// How wide it shows, in logical pixels.
    width: f32,
    /// How tall it shows, in logical pixels.
    height: f32,
}

/// The visual treatment of a figure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FigureVariant {
    /// On its own, as drawn.
    #[default]
    Plain,
    /// On a sunken card, as diagrams sit.
    Framed,
}

/// A figure drawn from its source and shown at its own size, or a line
/// saying why it cannot be shown.
#[derive(Clone, Debug)]
pub struct Figure {
    /// What it is drawn from.
    source: FigureSource,
    /// Its visual treatment.
    variant: FigureVariant,
    /// The size of the line shown when it cannot be drawn.
    size: Size,
}

impl Figure {
    /// A figure drawn from `source`.
    pub fn new(source: FigureSource) -> Self {
        Self {
            source,
            variant: FigureVariant::Plain,
            size: Size::Sm,
        }
    }

    /// The figure with `variant`.
    pub fn variant(mut self, variant: FigureVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The figure with its failure line at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the figure.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let drawn = drawing(&self.source, &scheme, &extended);
        let size = self.size;
        let body = move || match &drawn {
            Ok(drawing) => image(
                Modifier::new()
                    .width_in(0.0, drawing.width)
                    .aspect_ratio(drawing.width / drawing.height.max(1.0)),
                &drawing.image,
                ContentScale::Fit,
            ),
            Err(reason) => row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(crate::spacing(6.0)),
                Alignment::Center,
                || {
                    icon(phosphor::IMAGE_BROKEN, 14.0, scheme.on_surface_variant);
                    Label::new(reason.clone())
                        .variant(LabelVariant::Subtle)
                        .size(size)
                        .wrap()
                        .show();
                },
            ),
        };
        match self.variant {
            FigureVariant::Plain => column_with(Modifier::new(), body),
            FigureVariant::Framed => Card::new()
                .variant(CardVariant::Sunken)
                .size(Size::Md)
                .modifier(Modifier::new().fill_max_width())
                .show(move || {
                    row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::Center,
                        Alignment::Center,
                        body,
                    )
                }),
        }
    }
}

/// Whether a code fence naming `language` holds a diagram.
pub fn is_diagram(language: &str) -> bool {
    language.eq_ignore_ascii_case("mermaid")
}

/// Whether a code fence naming `language` holds a formula.
pub fn is_formula(language: &str) -> bool {
    language.eq_ignore_ascii_case("math") || language.eq_ignore_ascii_case("latex")
}

/// The drawing of `source` in the colours of `scheme`, drawn the first time it is asked for.
fn drawing(
    source: &FigureSource,
    scheme: &ColorScheme,
    extended: &ExtendedColors,
) -> Result<Rc<Drawing>, String> {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hex(scheme.on_surface).hash(&mut hasher);
    hex(scheme.background).hash(&mut hasher);
    let key = hasher.finish();
    if let Some(found) = DRAWN.with_borrow(|drawn| drawn.get(&key).cloned()) {
        return found;
    }
    let made = match source {
        FigureSource::Diagram(text) => diagram(text, scheme, extended),
        FigureSource::Formula(text) => formula(text, scheme),
        FigureSource::Image { source, alt } => picture(source, alt),
    }
    .map(Rc::new);
    DRAWN.with_borrow_mut(|drawn| drawn.insert(key, made.clone()));
    made
}

/// `colour` written as `#rrggbb`.
fn hex(colour: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.r, colour.g, colour.b)
}

/// Whether `colour` is dark.
fn is_dark(colour: Color) -> bool {
    u32::from(colour.r) + u32::from(colour.g) + u32::from(colour.b) < 3 * 128
}

/// The diagram written in mermaid as `text`, in the colours of `scheme`.
fn diagram(text: &str, scheme: &ColorScheme, extended: &ExtendedColors) -> Result<Drawing, String> {
    let mut theme = if is_dark(scheme.background) {
        mermaid_svg::Theme::dark()
    } else {
        mermaid_svg::Theme::default_theme()
    };
    let variables: BTreeMap<String, String> = [
        ("background", scheme.background),
        ("primaryColor", scheme.surface_container),
        ("mainBkg", scheme.surface_container),
        ("secondaryColor", scheme.surface_container_high),
        ("tertiaryColor", scheme.surface_container_highest),
        ("primaryBorderColor", extended.subtle),
        ("nodeBorder", extended.subtle),
        ("primaryTextColor", scheme.on_surface),
        ("textColor", scheme.on_surface),
        ("titleColor", scheme.on_surface),
        ("lineColor", scheme.on_surface_variant),
        ("signalColor", scheme.on_surface_variant),
        ("signalTextColor", scheme.on_surface),
        ("clusterBkg", scheme.surface_container_low),
        ("clusterBorder", scheme.outline_variant),
        ("edgeLabelBackground", scheme.background),
    ]
    .into_iter()
    .map(|(name, colour)| (name.to_owned(), hex(colour)))
    .collect();
    theme.apply_theme_variables(&variables);
    theme.responsive = false;
    let svg = mermaid_svg::render_with(text, &theme)
        .map_err(|error| format!("This diagram could not be drawn: {error}"))?;
    vector(&svg)
}

/// The formula written in TeX as `text`, set in the text colour of `scheme`.
fn formula(text: &str, scheme: &ColorScheme) -> Result<Drawing, String> {
    let font = latex_rust::MathFont::stix_two_math().map_err(|error| error.to_string())?;
    let mut options = latex_rust::SvgOptions::new();
    options.font_size_pt = latex_rust::Dim::ratio(FORMULA_POINTS.0, FORMULA_POINTS.1);
    options.color = latex_rust::Color::rgb(
        scheme.on_surface.r,
        scheme.on_surface.g,
        scheme.on_surface.b,
    );
    options.display = true;
    let svg = latex_rust::latex_to_svg(text, &font, &options)
        .map_err(|error| format!("This formula could not be set: {error}"))?;
    vector(&svg)
}

/// The image at `source`: a `data:` address is decoded here; other addresses
/// say the picture is not loaded, naming it by `alt`.
fn picture(source: &str, alt: &str) -> Result<Drawing, String> {
    let named = if alt.is_empty() { source } else { alt };
    let Some(data) = source.strip_prefix("data:") else {
        return Err(format!("{named} is not loaded"));
    };
    let (kind, payload) = data
        .split_once(',')
        .ok_or_else(|| format!("{named} could not be read"))?;
    let bytes = if kind.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|error| format!("{named} could not be read: {error}"))?
    } else {
        payload.as_bytes().to_vec()
    };
    if kind.starts_with("image/svg") {
        return vector(&String::from_utf8_lossy(&bytes));
    }
    let image =
        ImageData::decode(&bytes).map_err(|error| format!("{named} could not be read: {error}"))?;
    let (width, height) = (image.width() as f32, image.height() as f32);
    Ok(Drawing {
        image,
        width,
        height,
    })
}

/// The vector picture `svg` drawn at [`DENSITY`].
fn vector(svg: &str) -> Result<Drawing, String> {
    let options = resvg::usvg::Options {
        fontdb: Arc::clone(&FONTS),
        ..resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|error| format!("This picture could not be drawn: {error}"))?;
    let size = tree.size();
    let longest = size.width().max(size.height()).max(1.0);
    let scale = (longest * DENSITY).min(LONGEST_SIDE) / longest;
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut canvas = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "This picture has no size to draw.".to_owned())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut canvas.as_mut(),
    );
    let pixels = canvas
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let colour = pixel.demultiply();
            [colour.red(), colour.green(), colour.blue(), colour.alpha()]
        })
        .collect();
    let image = ImageData::from_rgba(width, height, pixels)
        .ok_or_else(|| "This picture could not be drawn.".to_owned())?;
    Ok(Drawing {
        image,
        width: size.width() * scale / DENSITY,
        height: size.height() * scale / DENSITY,
    })
}

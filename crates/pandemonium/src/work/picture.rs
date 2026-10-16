//! Pictures of open files turned into pixels away from the window: picture files decoded from their bytes and vector pictures drawn from their text.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

use pm_color::Role;
use pm_components::figures::Drawing;
use pm_components::preview::VECTOR_DENSITY;
use pm_renderer::Picture;

/// The longest side a picture is drawn or kept at, in pixels, so its texture fits any GPU.
const LONGEST_SIDE: u32 = 4096;

/// The shortest a vector picture's longest side is drawn at, in pixels, so a small icon is shown large enough to see.
const SHORTEST_VECTOR_SIDE: f32 = 512.0;

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

/// The colours diagrams and formulas are drawn in, taken from the theme the window is drawn in.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Palette {
    /// Whether the theme is a dark one.
    dark: bool,
    /// The colours, as the theme variables of a diagram name them, in `#rrggbb`.
    variables: BTreeMap<String, String>,
    /// The colour of running text, red, green and blue, which formulas are set in.
    ink: [u8; 3],
}

impl Palette {
    /// The colours of `theme`, a dark one when `dark`.
    pub(crate) fn of(theme: &pm_theme::Theme, dark: bool) -> Self {
        let hex = |role: Role| {
            let [red, green, blue, _] = pm_color::Roles::role(theme, role).to_bytes();
            format!("#{red:02x}{green:02x}{blue:02x}")
        };
        let variables = [
            ("background", Role::Background),
            ("primaryColor", Role::Surface),
            ("mainBkg", Role::Surface),
            ("secondaryColor", Role::SurfaceHover),
            ("tertiaryColor", Role::SurfaceActive),
            ("primaryBorderColor", Role::Subtle),
            ("nodeBorder", Role::Subtle),
            ("primaryTextColor", Role::Text),
            ("textColor", Role::Text),
            ("titleColor", Role::Text),
            ("lineColor", Role::Muted),
            ("signalColor", Role::Muted),
            ("signalTextColor", Role::Text),
            ("actorBkg", Role::Surface),
            ("actorBorder", Role::Subtle),
            ("actorTextColor", Role::Text),
            ("actorLineColor", Role::Muted),
            ("labelBoxBkgColor", Role::Surface),
            ("noteBkgColor", Role::SurfaceSelected),
            ("noteBorderColor", Role::Border),
            ("clusterBkg", Role::Sidebar),
            ("clusterBorder", Role::Border),
            ("edgeLabelBackground", Role::Background),
        ]
        .into_iter()
        .map(|(name, role)| (name.to_owned(), hex(role)))
        .collect();
        let [red, green, blue, _] = pm_color::Roles::role(theme, Role::Text).to_bytes();
        Self {
            dark,
            variables,
            ink: [red, green, blue],
        }
    }
}

/// A key naming pixels made from `source` for the file at `path`, so the same pixels are uploaded once.
fn key_of(path: &str, source: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (path, source).hash(&mut hasher);
    hasher.finish()
}

/// The picture `bytes` of the file at `path` hold, decoded and shrunk to fit the longest side kept, or why it could not be read.
pub(crate) fn decode(path: &str, bytes: &[u8], truncated: bool) -> Result<Picture, String> {
    if truncated {
        return Err("This picture is too large to show.".to_owned());
    }
    let decoded = image::load_from_memory(bytes)
        .map_err(|error| format!("This picture could not be read: {error}"))?;
    let decoded = if decoded.width().max(decoded.height()) > LONGEST_SIDE {
        decoded.thumbnail(LONGEST_SIDE, LONGEST_SIDE)
    } else {
        decoded
    };
    let rgba = decoded.to_rgba8();
    Ok(Picture {
        key: key_of(path, bytes),
        width: rgba.width(),
        height: rgba.height(),
        pixels: Arc::new(rgba.into_raw()),
    })
}

/// The vector picture `text` of the file at `path` holds, drawn at the density of a drawn vector picture and its longest side no shorter than the shortest a vector file is drawn at, or why it could not be drawn.
pub(crate) fn draw(path: &str, text: &str) -> Result<Picture, String> {
    draw_sized(path, text, SHORTEST_VECTOR_SIDE)
}

/// The vector picture `text` of the file at `path` holds, drawn at the density of a drawn vector picture and its longest side no shorter than `shortest` pixels, or why it could not be drawn.
fn draw_sized(path: &str, text: &str, shortest: f32) -> Result<Picture, String> {
    let options = resvg::usvg::Options {
        fontdb: Arc::clone(&FONTS),
        ..resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(text, &options)
        .map_err(|error| format!("This picture could not be drawn: {error}"))?;
    let size = tree.size();
    let longest = size.width().max(size.height()).max(1.0);
    let side = (longest * VECTOR_DENSITY).clamp(shortest, LONGEST_SIDE as f32);
    let scale = side / longest;
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
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Ok(Picture {
        key: key_of(path, text.as_bytes()),
        width,
        height,
        pixels: Arc::new(pixels),
    })
}

/// Whether `bytes` hold a vector picture's text rather than the pixels of a picture.
fn is_vector(bytes: &[u8]) -> bool {
    let start = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(start);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<') && text.contains("<svg")
}

/// The picture `bytes` hold, drawn from its text when it is a vector picture and decoded otherwise, or why it could not be.
pub(crate) fn figure(bytes: &[u8], truncated: bool) -> Result<Drawing, String> {
    if is_vector(bytes) {
        let text = String::from_utf8_lossy(bytes);
        return draw_sized("figure", &text, 1.0).map(|picture| Drawing {
            picture,
            density: VECTOR_DENSITY,
        });
    }
    decode("figure", bytes, truncated).map(|picture| Drawing {
        picture,
        density: 1.0,
    })
}

/// The diagram written in mermaid as `source`, drawn in the colours of `palette`, or why it could not be drawn.
pub(crate) fn diagram(source: &str, palette: &Palette) -> Result<Drawing, String> {
    let mut theme = if palette.dark {
        mermaid_svg::Theme::dark()
    } else {
        mermaid_svg::Theme::default_theme()
    };
    theme.apply_theme_variables(&palette.variables);
    theme.responsive = false;
    let svg = mermaid_svg::render_with(source, &theme)
        .map_err(|error| format!("This diagram could not be drawn: {error}"))?;
    draw_sized("diagram", &svg, 1.0).map(|picture| Drawing {
        picture,
        density: VECTOR_DENSITY,
    })
}

/// How tall the type of a formula is, in points, as the type of running text beside it is.
const FORMULA_POINTS: (i64, i64) = (27, 2);

/// The formula written in TeX as `source`, set on lines of its own in the ink of `palette`, or why it could not be set.
pub(crate) fn formula(source: &str, palette: &Palette) -> Result<Drawing, String> {
    let font = latex_rust::MathFont::stix_two_math().map_err(|error| error.to_string())?;
    let [red, green, blue] = palette.ink;
    let mut options = latex_rust::SvgOptions::new();
    options.font_size_pt = latex_rust::Dim::ratio(FORMULA_POINTS.0, FORMULA_POINTS.1);
    options.color = latex_rust::Color::rgb(red, green, blue);
    options.display = true;
    let svg = latex_rust::latex_to_svg(source, &font, &options)
        .map_err(|error| format!("This formula could not be set: {error}"))?;
    draw_sized("formula", &svg, 1.0).map(|picture| Drawing {
        picture,
        density: VECTOR_DENSITY,
    })
}

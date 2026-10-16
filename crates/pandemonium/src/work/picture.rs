//! Pictures of open files turned into pixels away from the window: picture files decoded from their bytes and vector pictures drawn from their text.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

use pm_color::Role;
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

/// The colours diagrams are drawn in, taken from the theme the window is drawn in.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Palette {
    /// Whether the theme is a dark one.
    dark: bool,
    /// The colours, as the theme variables of a diagram name them, in `#rrggbb`.
    variables: BTreeMap<String, String>,
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
        Self { dark, variables }
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

/// The vector picture `text` of the file at `path` holds, drawn at the density of a drawn vector picture, or why it could not be drawn.
pub(crate) fn draw(path: &str, text: &str) -> Result<Picture, String> {
    let options = resvg::usvg::Options {
        fontdb: Arc::clone(&FONTS),
        ..resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(text, &options)
        .map_err(|error| format!("This picture could not be drawn: {error}"))?;
    let size = tree.size();
    let longest = size.width().max(size.height()).max(1.0);
    let side = (longest * VECTOR_DENSITY).clamp(SHORTEST_VECTOR_SIDE, LONGEST_SIDE as f32);
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

/// The diagram written in mermaid as `source`, drawn in the colours of `palette`, or why it could not be drawn.
pub(crate) fn diagram(source: &str, palette: &Palette) -> Result<Picture, String> {
    let mut theme = if palette.dark {
        mermaid_svg::Theme::dark()
    } else {
        mermaid_svg::Theme::default_theme()
    };
    theme.apply_theme_variables(&palette.variables);
    theme.responsive = false;
    let svg = mermaid_svg::render_with(source, &theme)
        .map_err(|error| format!("This diagram could not be drawn: {error}"))?;
    draw("diagram", &svg)
}

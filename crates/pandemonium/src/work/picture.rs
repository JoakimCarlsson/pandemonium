//! Pictures of open files turned into pixels away from the window: picture files decoded from their bytes and vector pictures drawn from their text with the interface's fonts.

use std::collections::BTreeMap;

use pm_components::figures::Drawing;
use pm_components::preview::VECTOR_DENSITY;
use pm_components::theme::Role;
use rustle::{ImageData, SvgPainter};

/// The longest side a picture is drawn or kept at, in pixels, so its texture fits any GPU.
const LONGEST_SIDE: u32 = 4096;

/// The shortest a vector picture's longest side is drawn at, in pixels, so a small icon is shown large enough to see.
const SHORTEST_VECTOR_SIDE: f32 = 512.0;

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
    pub(crate) fn of(theme: &pm_components::theme::Theme, dark: bool) -> Self {
        let hex = |role: Role| {
            let color = theme.color(role);
            format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
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
        let text = theme.color(Role::Text);
        Self {
            dark,
            variables,
            ink: [text.r, text.g, text.b],
        }
    }
}

/// The picture `bytes` hold, decoded and shrunk to fit the longest side kept, or why it could not be read.
pub(crate) fn decode(bytes: &[u8], truncated: bool) -> Result<ImageData, String> {
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
    ImageData::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
        .ok_or_else(|| "This picture has no pixels to show.".to_owned())
}

/// The vector picture `text` holds, drawn at the density of a drawn vector picture and its longest side no shorter than the shortest a vector file is drawn at, or why it could not be drawn.
pub(crate) fn draw(text: &str) -> Result<ImageData, String> {
    draw_sized(text, SHORTEST_VECTOR_SIDE)
}

/// The vector picture `text` holds, drawn at the density of a drawn vector picture and its longest side no shorter than `shortest` pixels, or why it could not be drawn.
///
/// # Errors
///
/// A message saying the text is not a vector picture, or that it has no size to draw.
fn draw_sized(text: &str, shortest: f32) -> Result<ImageData, String> {
    let painter = SvgPainter::from_str(text)
        .map_err(|error| format!("This picture could not be drawn: {error}"))?;
    let (natural_width, natural_height) = painter.intrinsic_size();
    let longest = natural_width.max(natural_height).max(1.0);
    let side = (longest * VECTOR_DENSITY).clamp(shortest, LONGEST_SIDE as f32);
    let scale = side / longest;
    let width = (natural_width * scale).round().max(1.0) as u32;
    let height = (natural_height * scale).round().max(1.0) as u32;
    let framed = SvgPainter::from_str(&framed(
        text,
        width,
        height,
        scale,
        (natural_width, natural_height),
    ))
    .map_err(|error| format!("This picture could not be drawn: {error}"))?;
    framed
        .rasterize(width, height)
        .ok_or_else(|| "This picture has no size to draw.".to_owned())
}

/// A vector picture `width` by `height` pixels at `scale` pixels a unit that holds the
/// picture `text`, `natural` units in size, at its top left: drawn at those pixels, the
/// picture is scaled alike across and down, and what does not fill a whole pixel is cut off
/// at the right and bottom edges or left empty there.
fn framed(text: &str, width: u32, height: u32, scale: f32, natural: (f32, f32)) -> String {
    let encoded: String = text
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}"><image width="{}" height="{}" preserveAspectRatio="none" xlink:href="data:image/svg+xml,{encoded}"/></svg>"#,
        width as f32 / scale,
        height as f32 / scale,
        natural.0,
        natural.1,
    )
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
        return draw_sized(&text, 1.0).map(|picture| Drawing {
            picture,
            density: VECTOR_DENSITY,
        });
    }
    decode(bytes, truncated).map(|picture| Drawing {
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
    draw_sized(&svg, 1.0).map(|picture| Drawing {
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
    draw_sized(&svg, 1.0).map(|picture| Drawing {
        picture,
        density: VECTOR_DENSITY,
    })
}

#[cfg(test)]
/// Checks that vector pictures are drawn with their text.
mod tests {
    use super::figure;

    /// A badge with the words `ci` and `passing` on it.
    const LABELLED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="88" height="20"><rect width="88" height="20" rx="3" fill="#555"/><rect x="34" width="54" height="20" rx="3" fill="#4c1"/><text x="6" y="14" fill="#fff" font-family="sans-serif" font-size="11">ci</text><text x="40" y="14" fill="#fff" font-family="sans-serif" font-size="11">passing</text></svg>"##;

    /// The same badge without its words.
    const BLANK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="88" height="20"><rect width="88" height="20" rx="3" fill="#555"/><rect x="34" width="54" height="20" rx="3" fill="#4c1"/></svg>"##;

    /// The text of a vector picture is set with the interface's fonts once the interface has them.
    #[test]
    fn vector_picture_text_is_drawn() {
        let _interface = rustle::Embedded::without_renderer(|| {}, || {});
        let labelled = figure(LABELLED.as_bytes(), false).expect("the badge is drawn");
        let blank = figure(BLANK.as_bytes(), false).expect("the blank badge is drawn");
        assert_eq!(
            (labelled.picture.width(), labelled.picture.height()),
            (blank.picture.width(), blank.picture.height())
        );
        let lettered = labelled
            .picture
            .pixels()
            .chunks(4)
            .zip(blank.picture.pixels().chunks(4))
            .filter(|(with, without)| with != without)
            .count();
        assert!(lettered > 20, "only {lettered} pixels show the words");
    }
}

//! The module of the Math language: it sets a formula written in TeX as an SVG picture, in the ink of the
//! theme the host says it is drawn for.

use std::collections::BTreeMap;

use pm_plugin_api::{LanguageCall, LanguageReply, pack};

/// What the formulas are written in, as a fence names it.
const NAME: &str = "math";

/// How tall the type of a formula is, in points, as the type of running text beside it is.
const FORMULA_POINTS: (i64, i64) = (27, 2);

/// The red, green and blue of the colour `text`, `#rrggbb`, or black.
fn ink(text: Option<&String>) -> [u8; 3] {
    let digits = text
        .map(|held| held.trim_start_matches('#'))
        .unwrap_or_default();
    let byte = |at: usize| {
        digits
            .get(at..at + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .unwrap_or(0)
    };
    [byte(0), byte(2), byte(4)]
}

/// The formula `source` set on lines of its own in the ink of the theme, as an SVG picture, or why it could not be set.
fn draw(source: &str, _dark: bool, colors: &BTreeMap<String, String>) -> Result<String, String> {
    let font = latex_rust::MathFont::stix_two_math().map_err(|error| error.to_string())?;
    let [red, green, blue] = ink(colors.get("textColor"));
    let mut options = latex_rust::SvgOptions::new();
    options.font_size_pt = latex_rust::Dim::ratio(FORMULA_POINTS.0, FORMULA_POINTS.1);
    options.color = latex_rust::Color::rgb(red, green, blue);
    options.display = true;
    latex_rust::latex_to_svg(source, &font, &options)
        .map_err(|error| format!("This formula could not be set: {error}"))
}

/// Hands the host `length` bytes of this module's memory to write into.
#[unsafe(no_mangle)]
pub extern "C" fn pm_alloc(length: usize) -> *mut u8 {
    let mut room = Vec::<u8>::with_capacity(length.max(1));
    let pointer = room.as_mut_ptr();
    std::mem::forget(room);
    pointer
}

/// Answers a language call: reads it from the memory the host wrote it to and leaves the reply there.
#[unsafe(no_mangle)]
pub extern "C" fn pm_language(pointer: *const u8, length: usize) -> i64 {
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let reply = match serde_json::from_slice::<LanguageCall>(bytes) {
        Ok(LanguageCall::Render {
            language,
            source,
            dark,
            colors,
        }) if language.eq_ignore_ascii_case(NAME) => match draw(&source, dark, &colors) {
            Ok(svg) => LanguageReply::Svg(svg),
            Err(reason) => LanguageReply::Error(reason),
        },
        _ => LanguageReply::Unsupported,
    };
    let bytes = serde_json::to_vec(&reply)
        .unwrap_or_default()
        .into_boxed_slice();
    let length = bytes.len() as u32;
    let pointer = Box::leak(bytes).as_ptr() as u32;
    pack(pointer, length)
}

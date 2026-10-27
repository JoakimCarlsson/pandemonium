//! The module of the Mermaid language: it draws a diagram written in mermaid as an SVG picture, in the
//! colours of the theme the host says it is drawn for.

use std::collections::BTreeMap;

use pm_plugin_api::{LanguageCall, LanguageReply, pack};

/// What the diagrams are written in, as a fence names it.
const NAME: &str = "mermaid";

/// The diagram `source` as an SVG picture, in the colours `colors` name, or why it could not be drawn.
fn draw(source: &str, dark: bool, colors: &BTreeMap<String, String>) -> Result<String, String> {
    let mut theme = if dark {
        mermaid_svg::Theme::dark()
    } else {
        mermaid_svg::Theme::default_theme()
    };
    theme.apply_theme_variables(colors);
    theme.responsive = false;
    mermaid_svg::render_with(source, &theme)
        .map_err(|error| format!("This diagram could not be drawn: {error}"))
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

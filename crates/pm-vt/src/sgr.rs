//! Select Graphic Rendition: the `CSI m` parameters that style later cells.

use vte::Params;

use crate::cell::{Attrs, Color};

/// Folds one SGR sequence into `attrs`.
///
/// The extended colour forms come in two spellings — `38;5;n` and `38:5:n` —
/// and both reach here as the same flattened list, so one walk over it reads
/// either.
pub fn apply(attrs: &mut Attrs, params: &Params) {
    let flat = flatten(params);
    let mut index = 0;
    while index < flat.len() {
        let consumed = match flat[index] {
            38 => match color_at(&flat[index..]) {
                Some((color, consumed)) => {
                    attrs.foreground = color;
                    consumed
                }
                None => flat.len(),
            },
            48 => match color_at(&flat[index..]) {
                Some((color, consumed)) => {
                    attrs.background = color;
                    consumed
                }
                None => flat.len(),
            },
            code => {
                simple(attrs, code);
                1
            }
        };
        index += consumed.max(1);
    }
}

/// Applies one parameter that names a style or a palette colour.
fn simple(attrs: &mut Attrs, code: u16) {
    match code {
        0 => *attrs = Attrs::DEFAULT,
        1 => attrs.bold = true,
        2 => attrs.dim = true,
        3 => attrs.italic = true,
        4 => attrs.underline = true,
        7 => attrs.inverse = true,
        8 => attrs.hidden = true,
        9 => attrs.strikethrough = true,
        21 | 22 => {
            attrs.bold = false;
            attrs.dim = false;
        }
        23 => attrs.italic = false,
        24 => attrs.underline = false,
        27 => attrs.inverse = false,
        28 => attrs.hidden = false,
        29 => attrs.strikethrough = false,
        30..=37 => attrs.foreground = Color::Indexed((code - 30) as u8),
        39 => attrs.foreground = Color::Default,
        40..=47 => attrs.background = Color::Indexed((code - 40) as u8),
        49 => attrs.background = Color::Default,
        90..=97 => attrs.foreground = Color::Indexed((code - 90 + 8) as u8),
        100..=107 => attrs.background = Color::Indexed((code - 100 + 8) as u8),
        _ => {}
    }
}

/// Reads the colour a `38` or `48` parameter introduces, and its length.
fn color_at(rest: &[u16]) -> Option<(Color, usize)> {
    match rest.get(1)? {
        2 => {
            let channel = |index: usize| rest.get(index).copied().unwrap_or(0).min(255) as u8;
            Some((Color::Rgb(channel(2), channel(3), channel(4)), 5))
        }
        5 => Some((
            Color::Indexed(rest.get(2).copied().unwrap_or(0).min(255) as u8),
            3,
        )),
        _ => None,
    }
}

/// The parameters and their subparameters as one list, in the order written.
fn flatten(params: &Params) -> Vec<u16> {
    params.iter().flatten().copied().collect()
}

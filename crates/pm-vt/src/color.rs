//! The 256-colour palette, for the entries a theme does not name itself.

/// The channels of palette entry `index`, for the entries above the first 16.
///
/// The first 16 are the theme's: they are the colours a user recognises as
/// "red" or "bright black", and a terminal that hard-codes them ignores the
/// theme it is drawn in. Everything above them is the cube and the greyscale
/// ramp, which are arithmetic and the same everywhere.
pub fn palette(index: u8) -> (u8, u8, u8) {
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => cube(index - 16),
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            (level, level, level)
        }
    }
}

/// The colour of cube entry `index`, counted from the first cube colour.
fn cube(index: u8) -> (u8, u8, u8) {
    let level = |step: u8| if step == 0 { 0 } else { 55 + step * 40 };
    (level(index / 36), level((index / 6) % 6), level(index % 6))
}

/// The first 16 entries as a terminal has always defined them.
///
/// Used only when nothing else names them; a theme that has its own palette
/// overrides these before a cell is drawn.
const ANSI: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00),
    (0xcd, 0x00, 0x00),
    (0x00, 0xcd, 0x00),
    (0xcd, 0xcd, 0x00),
    (0x00, 0x00, 0xee),
    (0xcd, 0x00, 0xcd),
    (0x00, 0xcd, 0xcd),
    (0xe5, 0xe5, 0xe5),
    (0x7f, 0x7f, 0x7f),
    (0xff, 0x00, 0x00),
    (0x00, 0xff, 0x00),
    (0xff, 0xff, 0x00),
    (0x5c, 0x5c, 0xff),
    (0xff, 0x00, 0xff),
    (0x00, 0xff, 0xff),
    (0xff, 0xff, 0xff),
];

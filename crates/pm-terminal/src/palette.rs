//! The 256 colours a terminal program can name by number.

/// The sixteen colours the first entries name, as red, green and blue.
pub const ANSI: [(u8, u8, u8); 16] = [
    (0x1a, 0x1b, 0x1c),
    (0xd9, 0x80, 0x6f),
    (0x6f, 0xb9, 0x8a),
    (0xd9, 0xb8, 0x6f),
    (0x6f, 0x9f, 0xd9),
    (0xb1, 0x86, 0xd9),
    (0x6f, 0xc4, 0xc4),
    (0xd6, 0xd6, 0xd6),
    (0x66, 0x66, 0x66),
    (0xe8, 0x9a, 0x8a),
    (0x8c, 0xd1, 0xa4),
    (0xe8, 0xcc, 0x8a),
    (0x8c, 0xb8, 0xe8),
    (0xc6, 0xa2, 0xe8),
    (0x8c, 0xd6, 0xd6),
    (0xff, 0xff, 0xff),
];

/// The red, green and blue of entry `index`: the sixteen above, a 6 by 6 by 6 cube, then 24 greys.
pub fn indexed(index: u8) -> (u8, u8, u8) {
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            let cube = index - 16;
            let level = |part: u8| if part == 0 { 0 } else { 55 + part * 40 };
            (level(cube / 36), level(cube / 6 % 6), level(cube % 6))
        }
        _ => {
            let grey = 8 + (index - 232) * 10;
            (grey, grey, grey)
        }
    }
}

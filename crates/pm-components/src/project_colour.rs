//! The colour a project is known by, derived from its path.

use rustle::Color;

use crate::app_theme::extended_colors;

/// FNV-1a offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The colour of the project at `path`, hashed from the path so the same
/// project keeps its colour everywhere, from the extended colours in effect.
pub fn project_colour(path: &str) -> Color {
    let hash = path.bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    let colours = extended_colors().projects;
    colours[(hash % colours.len() as u64) as usize]
}

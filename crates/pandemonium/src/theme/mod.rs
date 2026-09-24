//! Themes: the families on offer, what each one paints, and the names its
//! colours and emphases go by.
//!
//! The shape is the keymap's. Every family on offer is a [`ThemeFile`], the
//! editor's own and the reader's alike: a name, the family it builds on and
//! the [`Paint`] it lays over that family in either appearance. The families
//! are resolved once when they are put on offer, so a frame reads a finished
//! [`pm_ui::ThemeFamily`] and never walks a chain of files.

mod offered;
mod paint;
mod tokens;
mod weights;

pub use offered::{DEFAULT_FAMILY, ThemeFile, families, family, find, install};
pub use paint::{ANSI, Paint};
pub use tokens::{Group, TOKENS, from_hex, hex, in_group, token};
pub use weights::weight;

//! Every colour of a theme, by the name a theme file and the settings pane
//! call it.
//!
//! A theme file, the reader's overrides and the settings pane all name
//! colours the same way — a group and a key within it — so the names live in
//! one table rather than in three matches that drift apart.

use pm_gfx::Rgba;
use pm_ui::Theme;

/// Which part of a theme a colour belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    /// The semantic colours the window's furniture is painted in.
    Colors,
    /// The colours code is highlighted in.
    Syntax,
    /// The colours a terminal grid is drawn in, beside its palette.
    Terminal,
}

impl Group {
    /// Every group, in the order the settings pane lists them.
    pub const ALL: [Self; 3] = [Self::Colors, Self::Syntax, Self::Terminal];

    /// The heading the settings pane lists the group's colours under.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Colors => "Interface",
            Self::Syntax => "Syntax",
            Self::Terminal => "Terminal",
        }
    }
}

/// One colour of a theme, and how to read and write it.
#[derive(Clone, Copy, Debug)]
pub struct Token {
    /// The part of the theme it belongs to.
    pub group: Group,
    /// Its name within that part, as a theme file writes it.
    pub key: &'static str,
    /// Reads it off a theme.
    read: fn(&Theme) -> Rgba,
    /// Writes it into a theme.
    write: fn(&mut Theme, Rgba),
}

impl Token {
    /// This colour in `theme`.
    pub fn read(&self, theme: &Theme) -> Rgba {
        (self.read)(theme)
    }

    /// Paints this colour of `theme` in `color`.
    pub fn write(&self, theme: &mut Theme, color: Rgba) {
        (self.write)(theme, color);
    }

    /// Its key as a heading: `surface_hover` as "Surface Hover".
    pub fn label(&self) -> String {
        self.key
            .split('_')
            .map(|word| {
                let mut letters = word.chars();
                letters.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(letters).collect()
                })
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Declares [`TOKENS`] from one line per colour: its group, the field of
/// [`Theme`] holding the group, and its key.
macro_rules! tokens {
    ($($group:ident $field:ident $key:ident),* $(,)?) => {
        /// Every colour a theme names, in the order the settings pane lists them.
        pub const TOKENS: &[Token] = &[$(Token {
            group: Group::$group,
            key: stringify!($key),
            read: |theme| theme.$field.$key,
            write: |theme, color| theme.$field.$key = color,
        }),*];
    };
}

tokens! {
    Colors colors background,
    Colors colors surface,
    Colors colors surface_hover,
    Colors colors surface_active,
    Colors colors surface_selected,
    Colors colors border,
    Colors colors border_variant,
    Colors colors border_focused,
    Colors colors border_selected,
    Colors colors drop_target,
    Colors colors text,
    Colors colors text_muted,
    Colors colors text_subtle,
    Colors colors text_on_accent,
    Colors colors cursor,
    Colors colors selection,
    Colors colors link,
    Colors colors accent,
    Colors colors accent_hover,
    Colors colors accent_active,
    Colors colors success,
    Colors colors warning,
    Colors colors danger,
    Syntax syntax keyword,
    Syntax syntax string,
    Syntax syntax function,
    Syntax syntax comment,
    Syntax syntax number,
    Syntax syntax type_name,
    Syntax syntax punctuation,
    Syntax syntax variable,
    Syntax syntax property,
    Syntax syntax constant,
    Syntax syntax operator,
    Syntax syntax tag,
    Syntax syntax attribute,
    Terminal terminal cursor,
    Terminal terminal selection,
}

/// The index into [`TOKENS`] of the colour `key` names in `group`.
pub fn token(group: Group, key: &str) -> Option<usize> {
    TOKENS
        .iter()
        .position(|token| token.group == group && token.key == key)
}

/// The indices into [`TOKENS`] of every colour in `group`.
pub fn in_group(group: Group) -> impl Iterator<Item = usize> {
    TOKENS
        .iter()
        .enumerate()
        .filter(move |(_, token)| token.group == group)
        .map(|(index, _)| index)
}

/// `color` as a theme file writes it: `#rrggbb`, or `#rrggbbaa` when it is
/// translucent.
pub fn hex(color: Rgba) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (
        channel(color.r),
        channel(color.g),
        channel(color.b),
        channel(color.a),
    );
    match a {
        255 => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
    }
}

/// `"#rrggbb"` or `"#rrggbbaa"`, with or without the `#`, as a colour.
pub fn from_hex(hex: &str) -> Option<Rgba> {
    let digits = hex.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok().map(Rgba::hex),
        8 => u32::from_str_radix(digits, 16).ok().map(Rgba::hexa),
        _ => None,
    }
}

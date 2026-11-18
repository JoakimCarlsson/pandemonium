//! Text: how it is set ([`Type`]), lines of it, wrapped paragraphs, and
//! measuring and wrapping outside composition.
//!
//! Text in the old interface was a box holding one unwrapped line, set in
//! 16 pixel type with a line height of one and a half unless its own
//! utilities said otherwise; it never inherited from its parent. [`Type`]
//! keeps that: every text names its own size, line height, weight, family and
//! tone. [`label`] draws one line as before, cut short with an ellipsis when
//! [`Type::truncate`] says so; [`paragraph`] wraps at the width it is given,
//! which replaces building one line node per wrapped line.

use rustle::{FontWeight, Modifier, TextLayout, TextStyle, TextWrap};

use crate::kit::theme::{self, Role, Tone};

/// How a stretch of text is set: size, line height, weight, family, slant,
/// decoration, tone, and whether it wraps or is cut short.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Type {
    /// The size in logical pixels before the text scale.
    size: f32,
    /// Whether the size follows the reader's text scale, as interface text does and code does not.
    scaled: bool,
    /// The distance between baselines, as a share of the size.
    ratio: f32,
    /// The distance between baselines in logical pixels, when fixed rather than a share of the size.
    leading: Option<f32>,
    /// The weight on the 100 to 900 scale.
    weight: u16,
    /// Whether it is set in the code family.
    mono: bool,
    /// Whether it leans.
    italic: bool,
    /// Whether it is underlined.
    underline: bool,
    /// Whether it is struck through.
    strike: bool,
    /// Its colour.
    tone: Tone,
    /// Whether a line too long for its box ends in an ellipsis.
    truncate: bool,
    /// Whether it wraps at the width of its box.
    wrap: bool,
    /// Extra space after every character, in logical pixels.
    tracking: f32,
}

impl Default for Type {
    /// Body text as the old interface set it: 16 pixels, a line height of one and a half, normal weight, in the text role.
    fn default() -> Self {
        Self {
            size: 16.0,
            scaled: true,
            ratio: 1.5,
            leading: None,
            weight: 400,
            mono: false,
            italic: false,
            underline: false,
            strike: false,
            tone: Tone::from(Role::Text),
            truncate: false,
            wrap: false,
            tracking: 0.0,
        }
    }
}

impl Type {
    /// Interface text `pixels` tall before the reader's text scale, with a line height of one and a half: `text_px(pixels)`.
    pub fn new(pixels: impl Into<f64>) -> Self {
        Self {
            size: pixels.into() as f32,
            ..Self::default()
        }
    }

    /// Code `pixels` tall in the code family, which the reader's text scale leaves alone: `code_px(pixels)` with `font_mono()`.
    pub fn code(pixels: impl Into<f64>) -> Self {
        Self {
            size: pixels.into() as f32,
            scaled: false,
            mono: true,
            ..Self::default()
        }
    }

    /// The same type `pixels` tall, leaving the text scale alone: `code_px(pixels)`.
    pub fn unscaled(mut self, pixels: impl Into<f64>) -> Self {
        self.size = pixels.into() as f32;
        self.scaled = false;
        self
    }

    /// A line height of `ratio` times the size: `leading_ratio(ratio)`.
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio;
        self.leading = None;
        self
    }

    /// A line height of `pixels` logical pixels.
    pub fn leading_px(mut self, pixels: f32) -> Self {
        self.leading = Some(pixels);
        self
    }

    /// Text in `tone`: `text_color(tone)`.
    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
        self.tone = tone.into();
        self
    }

    /// Weight 500: `font_medium()`.
    pub fn medium(self) -> Self {
        self.weight(500)
    }

    /// Weight 600: `font_semibold()`.
    pub fn semibold(self) -> Self {
        self.weight(600)
    }

    /// Weight 700: `font_bold()`.
    pub fn bold(self) -> Self {
        self.weight(700)
    }

    /// Weight `weight` on the 100 to 900 scale.
    pub fn weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    /// Set in the code family: `font_mono()`.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Leaning: `italic()`.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Underlined: `underline()`.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Struck through: `line_through()`.
    pub fn strike(mut self) -> Self {
        self.strike = true;
        self
    }

    /// Letters `pixels` further apart; negative brings them together: `tracking_tight()` is `-0.4`.
    pub fn tracking(mut self, pixels: f32) -> Self {
        self.tracking = pixels;
        self
    }

    /// A line too long for its box ends in an ellipsis: `truncate()`.
    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self.wrap = false;
        self
    }

    /// Lines break at the width of the box, between words where they can.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self.truncate = false;
        self
    }

    /// The size in logical pixels, after the text scale when it follows it.
    pub fn size(&self) -> f32 {
        if self.scaled {
            self.size * theme::text_scale()
        } else {
            self.size
        }
    }

    /// The distance between baselines in logical pixels.
    pub fn line_height(&self) -> f32 {
        self.leading.unwrap_or(self.size() * self.ratio)
    }

    /// The tone it is set in.
    pub fn tone_of(&self) -> Tone {
        self.tone
    }

    /// The rustle style for this type, resolved against the theme being composed with.
    pub fn style(&self) -> TextStyle {
        self.style_in(&theme::theme())
    }

    /// The rustle style for this type, resolved against `theme`.
    pub fn style_in(&self, theme: &theme::Theme) -> TextStyle {
        let family = if self.mono {
            theme::mono()
        } else {
            theme::sans()
        };
        TextStyle {
            color: theme::color_in(theme, self.tone),
            font_size: self.size(),
            line_height: self.line_height(),
            font_weight: FontWeight(self.weight),
            font_family: family,
            max_lines: self.truncate.then_some(1),
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strike,
            letter_spacing: self.tracking,
            wrap: if self.wrap {
                TextWrap::Word
            } else {
                TextWrap::None
            },
            ligatures: !self.mono || code_ligatures(),
            ..TextStyle::default()
        }
    }
}

thread_local! {
    /// Whether code text joins characters such as `->` into its font's ligatures.
    static CODE_LIGATURES: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Lets code text join characters such as `->` into the ligatures its font
/// has for them, or sets each character alone: `shaper.set_ligatures(on)`.
/// Compose again after changing it.
pub fn set_code_ligatures(on: bool) {
    CODE_LIGATURES.with(|held| held.set(on));
}

/// Whether code text joins characters into ligatures: `shaper.ligatures()`.
pub fn code_ligatures() -> bool {
    CODE_LIGATURES.with(std::cell::Cell::get)
}

/// A number that changes whenever text would be measured differently: fonts
/// installed or registered, or ligatures switched; `shaper.font_key()`, for
/// keys of drawing kept between frames.
pub fn font_key() -> u64 {
    rustle::font_epoch().wrapping_mul(2) + u64::from(code_ligatures())
}

/// Emits one line of `content` set in `kind`: `text(content).styles([...])`.
///
/// It is as wide as its text, or as wide as its box allows and cut short
/// with an ellipsis when `kind` truncates. A line break in the content starts
/// a new line.
#[track_caller]
pub fn label(content: impl Into<String>, kind: Type) {
    label_with(Modifier::new(), content, kind);
}

/// Emits one line of `content` set in `kind`, with `modifier` around it.
#[track_caller]
pub fn label_with(modifier: Modifier, content: impl Into<String>, kind: Type) {
    rustle::text_styled(modifier, content, kind.style());
}

/// Emits `content` wrapped at the width its box gives it, set in `kind`.
///
/// This replaces wrapping with the shaper and emitting one line node per
/// wrapped line. A blank line in the content is one line tall.
#[track_caller]
pub fn paragraph(modifier: Modifier, content: impl Into<String>, kind: Type) {
    rustle::text_styled(modifier, content, kind.wrap().style());
}

/// One line of wrapped text, as byte offsets into the text it was cut from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Line {
    /// Where the line starts.
    pub start: usize,
    /// Where it ends, after any spaces it keeps at the break but before a line break.
    pub end: usize,
    /// Where the next line starts: the same place when the line was cut for
    /// want of room, past the line break when the text had one.
    pub next: usize,
}

/// How wide and tall `content` is on one line set in `kind`, in logical pixels.
///
/// Works outside composition once a renderer exists; replaces `shaper.measure`.
pub fn measure(content: &str, kind: Type) -> (f32, f32) {
    let layout = rustle::layout_text(content, &kind.style(), None);
    let size = layout.size();
    (size.width, size.height.max(kind.line_height()))
}

/// The shaped layout of `content` set in `kind`, wrapped at `width` when given.
pub fn shape(content: &str, kind: Type, width: Option<f32>) -> TextLayout {
    let style = if width.is_some() {
        kind.wrap().style()
    } else {
        kind.style()
    };
    rustle::layout_text(content, &style, width)
}

/// The lines `content` breaks into at `width` logical pixels when set in
/// `kind`; replaces `pm_ui::wrap`. The lines tile the text and at least one is
/// always returned.
pub fn wrap_lines(content: &str, kind: Type, width: f32) -> Vec<Line> {
    lines_of(&shape(content, kind, Some(width.max(1.0))), content.len())
}

/// The lines of `layout`, the shaped form of a text `length` bytes long, as [`Line`]s.
pub fn lines_of(layout: &TextLayout, length: usize) -> Vec<Line> {
    let laid = layout.lines();
    if laid.is_empty() {
        return vec![Line {
            start: 0,
            end: 0,
            next: length + 1,
        }];
    }
    laid.iter()
        .enumerate()
        .map(|(index, line)| {
            let next = laid.get(index + 1).map_or(length + 1, |after| after.start);
            Line {
                start: line.start,
                end: line.end.min(next),
                next,
            }
        })
        .collect()
}

/// The installed font families, all of them and the monospaced ones, each
/// sorted: `shaper.families()`. Empty before a renderer exists.
pub fn font_families() -> (Vec<String>, Vec<String>) {
    (rustle::font_families(), rustle::monospace_font_families())
}

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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rustle::{FontWeight, Modifier, TextLayout, TextStyle, TextWrap};

use crate::kit::focus::FocusId;
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

    /// The same type without cutting its line short.
    fn untruncated(mut self) -> Self {
        self.truncate = false;
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
    ///
    /// Code is set in the code family only at the regular and bold weights,
    /// the faces code fonts have; code asked for at another weight, such as
    /// medium, is set in the interface family at that weight, as text always
    /// was here.
    pub fn style_in(&self, theme: &theme::Theme) -> TextStyle {
        let family = if self.mono && matches!(self.weight, 400 | 700) {
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
    let content: String = content.into();
    let scope = crate::kit::select::scope();
    if kind.truncate && !kind.underline && !kind.strike && !content.contains('\n') {
        return shortened(modifier, content, kind, scope);
    }
    if let Some(id) = scope {
        return crate::kit::select::selectable(modifier, id, content, kind);
    }
    rustle::text_styled(modifier, content, kind.style());
}

/// Emits one line of `content` set in `kind`, a piece of the selectable text
/// `scope` names when there is one, as wide as it is or as its box
/// allows, giving up room when its row runs out of it as a CSS flex item
/// does, cut short when it does not fit: the most of it that fits with an
/// ellipsis after it, the spaces before the ellipsis left out, as the old
/// interface cut its lines.
#[track_caller]
fn shortened(modifier: Modifier, content: String, kind: Type, scope: Option<FocusId>) {
    let style = TextStyle {
        max_lines: None,
        ..kind.style()
    };
    let natural = measure(&content, kind.untruncated()).0;
    let height = kind.line_height();
    let cut: RefCell<Option<(f32, String)>> = RefCell::new(None);
    let modifier = match scope {
        Some(id) => {
            let whole = rustle::layout_text(&content, &style, None);
            let shaped = Rc::new(RefCell::new(Some(Rc::new(whole))));
            crate::kit::select::piece(modifier, id, Rc::from(content.as_str()), shaped)
        }
        None => modifier,
    };
    rustle::layout(
        modifier.shrink(1.0),
        Shortened { natural, height },
        move || {
            rustle::canvas(Modifier::new().fill_max_size(), move |canvas| {
                let room = canvas.width();
                let mut held = cut.borrow_mut();
                if held.as_ref().is_none_or(|(width, _)| *width != room) {
                    *held = Some((room, ellipsized(&content, &style, room, 1.0)));
                }
                if let Some((_, words)) = held.as_ref() {
                    canvas.draw_text(0.0, 0.0, words, &style, None);
                }
            });
        },
    );
}

/// `content` set in `style`, cut short to the most of it that fits in `room`
/// logical pixels with an ellipsis after it, the spaces before the ellipsis
/// left out; `content` itself when it fits, or overflows by no more than
/// `slack` pixels, which a box sized to its line's rounded-up width allows.
pub(crate) fn ellipsized(content: &str, style: &TextStyle, room: f32, slack: f32) -> String {
    let width = |text: &str| rustle::layout_text(text, style, None).size().width;
    if width(content) <= room + slack {
        return content.to_owned();
    }
    let boundaries: Vec<usize> = content
        .char_indices()
        .map(|(at, _)| at)
        .chain(std::iter::once(content.len()))
        .collect();
    let (mut low, mut high) = (0, boundaries.len() - 1);
    while low < high {
        let middle = (low + high).div_ceil(2);
        let candidate = format!("{}…", content[..boundaries[middle]].trim_end());
        if width(&candidate) <= room {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    format!("{}…", content[..boundaries[low]].trim_end())
}

/// Lays out a line cut short at its box: as wide as its words when they fit,
/// else as wide as it may be, one line tall, its drawing filling it.
struct Shortened {
    /// How wide the whole line is.
    natural: f32,
    /// How tall a line is.
    height: f32,
}

impl rustle::Layout for Shortened {
    fn measure(
        &self,
        children: &mut rustle::Children,
        constraints: rustle::Constraints,
    ) -> rustle::LayoutResult {
        let size = rustle::Size::new(
            self.natural
                .ceil()
                .min(constraints.max_width)
                .max(constraints.min_width),
            self.height
                .min(constraints.max_height)
                .max(constraints.min_height),
        );
        for index in 0..children.len() {
            children.measure(index, rustle::Constraints::tight(size));
        }
        rustle::LayoutResult {
            size,
            positions: vec![rustle::Offset::new(0.0, 0.0); children.len()],
        }
    }

    fn min_intrinsic_width(&self, _: &mut rustle::Children, _: f32) -> f32 {
        0.0
    }

    fn max_intrinsic_width(&self, _: &mut rustle::Children, _: f32) -> f32 {
        self.natural.ceil()
    }

    fn min_intrinsic_height(&self, _: &mut rustle::Children, _: f32) -> f32 {
        self.height
    }

    fn max_intrinsic_height(&self, _: &mut rustle::Children, _: f32) -> f32 {
        self.height
    }
}

/// Emits `content` wrapped at the width its box gives it, set in `kind`.
///
/// This replaces wrapping with the shaper and emitting one line node per
/// wrapped line. A blank line in the content is one line tall.
#[track_caller]
pub fn paragraph(modifier: Modifier, content: impl Into<String>, kind: Type) {
    if let Some(id) = crate::kit::select::scope() {
        return crate::kit::select::selectable(modifier, id, content, kind.wrap());
    }
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
/// `kind`. The lines tile the text and at least one is always returned.
///
/// A line break always breaks a line, and lines otherwise break only at
/// spaces, each line keeping the spaces after its last word; a word wider
/// than the width is cut where it must be.
pub fn wrap_lines(content: &str, kind: Type, width: f32) -> Vec<Line> {
    let key = (
        crate::kit::text::font_key(),
        width.to_bits(),
        kind.size().to_bits(),
        kind.weight,
        kind.mono,
        kind.italic,
        kind.tracking.to_bits(),
    );
    if let Some(lines) =
        WRAPPED.with_borrow(|held| held.get(&key).and_then(|items| items.get(content)).cloned())
    {
        return lines;
    }
    let lines = wrap_uncached(content, kind, width);
    WRAPPED.with_borrow_mut(|held| {
        if held.values().map(HashMap::len).sum::<usize>() >= WRAPPED_MOST {
            held.clear();
        }
        held.entry(key)
            .or_default()
            .insert(content.to_owned(), lines.clone());
    });
    lines
}

/// How many wrapped texts are kept before the kept ones are dropped.
const WRAPPED_MOST: usize = 4096;

/// What a text is wrapped with besides its width: the fonts, the size, weight,
/// family, slant and letter spacing.
type WrapKey = (u64, u32, u32, u16, bool, bool, u32);

thread_local! {
    /// Texts wrapped so far, by what they were wrapped with.
    static WRAPPED: RefCell<HashMap<WrapKey, HashMap<String, Vec<Line>>>> = RefCell::new(HashMap::new());
}

/// The lines `content` breaks into at `width` when set in `kind`, measured afresh.
fn wrap_uncached(content: &str, kind: Type, width: f32) -> Vec<Line> {
    let style = TextStyle {
        max_lines: None,
        wrap: TextWrap::None,
        ..kind.style()
    };
    let mut fits = |piece: &str| {
        rustle::layout_text(piece.trim_end(), &style, None)
            .size()
            .width
            <= width
    };
    let mut lines = Vec::new();
    let mut paragraph = 0;
    for source in content.split('\n') {
        let end = paragraph + source.len();
        cut(source, paragraph, &mut fits, &mut lines);
        if let Some(last) = lines.last_mut() {
            last.next = end + 1;
        }
        paragraph = end + 1;
    }
    lines
}

/// Cuts the paragraph `source`, which starts at byte `base` of the text, into
/// lines that `fits`, breaking after the spaces that end words and cutting a
/// word too long for a line where it must be.
fn cut(source: &str, base: usize, fits: &mut impl FnMut(&str) -> bool, lines: &mut Vec<Line>) {
    let mut start = 0;
    let mut cursor = 0;
    for word_end in word_ends(source) {
        if word_end - start <= 256 && fits(&source[start..word_end]) {
            cursor = word_end;
            continue;
        }
        if cursor > start {
            lines.push(Line {
                start: base + start,
                end: base + cursor,
                next: base + cursor,
            });
            start = cursor;
        }
        loop {
            let split = start + fitting_prefix(&source[start..word_end], fits);
            if split >= word_end {
                break;
            }
            lines.push(Line {
                start: base + start,
                end: base + split,
                next: base + split,
            });
            start = split;
        }
        cursor = word_end;
    }
    lines.push(Line {
        start: base + start,
        end: base + source.len(),
        next: base + source.len(),
    });
}

/// The length of the longest start of `source` that `fits`, at least one
/// character, found with few measurements so a long unbroken word is not
/// measured at every length.
fn fitting_prefix(source: &str, fits: &mut impl FnMut(&str) -> bool) -> usize {
    let mut boundaries = Vec::with_capacity(64);
    let mut good = 0;
    let mut next = 1;
    for (at, character) in source.char_indices() {
        boundaries.push(at + character.len_utf8());
        if boundaries.len() == next || at + character.len_utf8() == source.len() {
            if fits(&source[..boundaries[boundaries.len() - 1]]) {
                good = boundaries.len();
                next *= 2;
            } else {
                break;
            }
        }
    }
    if good == boundaries.len() {
        return source.len();
    }
    let mut bad = boundaries.len();
    while good + 1 < bad {
        let middle = (good + bad) / 2;
        if fits(&source[..boundaries[middle - 1]]) {
            good = middle;
        } else {
            bad = middle;
        }
    }
    boundaries[good.max(1) - 1]
}

/// Where each word of `source` ends, the spaces after it taken along.
fn word_ends(source: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut spaced = false;
    for (at, character) in source.char_indices() {
        if character == ' ' {
            spaced = true;
        } else if spaced {
            ends.push(at);
            spaced = false;
        }
    }
    ends.push(source.len());
    ends
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Installs this thread's text engine, which measuring text needs.
    fn fonts() {
        drop(rustle::Embedded::without_renderer(|| {}, || {}));
    }

    /// Line breaks, blank paragraphs and a trailing line break keep the byte ranges of the text.
    #[test]
    fn wrapped_lines_keep_the_offsets_of_paragraphs() {
        fonts();
        let text = "é 🦀\n\nend\n";
        let line = |start, end, next| Line { start, end, next };
        assert_eq!(
            wrap_lines(text, Type::new(14), 1000.0),
            vec![
                line(0, 7, 8),
                line(8, 8, 9),
                line(9, 12, 13),
                line(13, 13, 14)
            ]
        );
    }

    /// Long text wrapped narrow is tiled by its lines without splitting a character.
    #[test]
    fn narrow_lines_tile_long_text() {
        fonts();
        let text = "café🦀 ".repeat(40);
        for width in [1.0, 24.0, 120.0] {
            let lines = wrap_lines(&text, Type::new(13), width);
            let mut covered = 0;
            for line in &lines {
                assert!(text.is_char_boundary(line.start) && text.is_char_boundary(line.end));
                assert_eq!(line.start, covered, "at width {width}");
                covered = line.next.min(text.len());
            }
            assert_eq!(covered, text.len());
        }
    }
}

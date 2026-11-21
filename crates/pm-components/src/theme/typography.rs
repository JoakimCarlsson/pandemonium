//! How text is set: the rustle [`TextStyle`]s of interface text and code, the
//! [`Typography`] widgets read, and the switches that change how text is shaped.
//!
//! Interface text follows the reader's text scale and is set in the theme's
//! sans family; code keeps its size and is set in the mono family, joining
//! characters into ligatures only while [`code_ligatures`] says so; the switch
//! is snapshot state, so the composables that set code read it and are composed
//! again when it changes. Every
//! style starts in the text role on one unwrapped line; callers change it
//! with rustle's own `TextStyle` methods.

use rustle::{Color, FontWeight, TextStyle, TextWrap, Typography};

use super::{Role, color, mono, sans, text_scale};

thread_local! {
    /// Whether code text joins characters such as `->` into its font's ligatures, as
    /// snapshot state that every composable setting code reads.
    static CODE_LIGATURES: rustle::State<bool> = rustle::mutable_state(true);
}

/// Lets code text join characters such as `->` into the ligatures its font
/// has for them, or sets each character alone; the composables that read the
/// switch are composed again when it changes.
pub fn set_code_ligatures(on: bool) {
    CODE_LIGATURES.with(|held| {
        if held.get() != on {
            held.set(on);
        }
    });
}

/// Whether code text joins characters into ligatures, read as snapshot state so
/// the composable reading it follows the switch.
pub fn code_ligatures() -> bool {
    CODE_LIGATURES.with(rustle::State::get)
}

/// A number that changes whenever text would be measured differently: fonts
/// installed or registered, or ligatures switched, for keys of drawing kept between frames.
pub fn font_key() -> u64 {
    rustle::font_epoch().wrapping_mul(2) + u64::from(code_ligatures())
}

/// The installed font families, all of them and the monospaced ones, each
/// sorted. Empty before a renderer exists.
pub fn font_families() -> (Vec<String>, Vec<String>) {
    (rustle::font_families(), rustle::monospace_font_families())
}

/// The style of one unwrapped line `pixels` tall with lines `ratio` times
/// that apart, in `family`, in the text role, joining ligatures while `ligatures` is true.
fn line_style(pixels: f32, ratio: f32, family: rustle::FontFamily, ligatures: bool) -> TextStyle {
    TextStyle {
        color: color(Role::Text),
        font_size: pixels,
        line_height: pixels * ratio,
        font_weight: FontWeight::NORMAL,
        font_family: family,
        ligatures,
        wrap: TextWrap::None,
        ..TextStyle::default()
    }
}

/// Interface text `pixels` tall before the reader's text scale, with lines
/// `ratio` times its size apart, in the sans family and the text role.
pub fn text_style(pixels: impl Into<f64>, ratio: f32) -> TextStyle {
    let size = pixels.into() as f32 * text_scale();
    line_style(size, ratio, sans(), true)
}

/// Interface text `pixels` tall that the reader's text scale leaves alone,
/// with lines `ratio` times its size apart, in the sans family.
pub fn fixed_text_style(pixels: impl Into<f64>, ratio: f32) -> TextStyle {
    line_style(pixels.into() as f32, ratio, sans(), true)
}

/// Interface text `pixels` tall before the reader's text scale, with lines
/// `ratio` times its size apart, set in the mono family.
pub fn mono_style(pixels: impl Into<f64>, ratio: f32) -> TextStyle {
    let size = pixels.into() as f32 * text_scale();
    line_style(size, ratio, mono(), code_ligatures())
}

/// Code `pixels` tall, which the reader's text scale leaves alone, with lines
/// `ratio` times its size apart, in the mono family.
pub fn code_style(pixels: impl Into<f64>, ratio: f32) -> TextStyle {
    line_style(pixels.into() as f32, ratio, mono(), code_ligatures())
}

/// How wide a character of code set at `pixels` is, in logical pixels: a run of wide
/// characters laid out in [`code_style`] and divided by its length, which rustle keeps
/// laid out while the fonts stay the same. Nothing wide before a renderer exists.
pub fn code_cell(pixels: f32) -> f32 {
    rustle::layout_text(&"M".repeat(64), &code_style(pixels, 1.5), None)
        .size()
        .width
        / 64.0
}

/// `style` set in the mono family at its weight when code fonts have that
/// face (regular and bold), and in the sans family otherwise, as code asked
/// for at another weight, such as medium, always was.
pub fn in_mono(style: TextStyle) -> TextStyle {
    if matches!(style.font_weight.0, 400 | 700) {
        TextStyle {
            font_family: mono(),
            ligatures: code_ligatures(),
            ..style
        }
    } else {
        style
    }
}

/// The text styles rustle's widgets read, by role, set in the app's type
/// scale: interface sizes in the sans family at one and a half line heights,
/// labels on one line, button labels medium at 1.4.
pub fn typography() -> Typography {
    let body = |pixels: f32| text_style(pixels, 1.5);
    let heading = |pixels: f32| text_style(pixels, 1.4).font_weight(FontWeight::SEMI_BOLD);
    Typography {
        display_large: heading(32.0),
        display_medium: heading(28.0),
        display_small: heading(24.0),
        headline_large: heading(22.0),
        headline_medium: heading(20.0),
        headline_small: heading(18.0),
        title_large: heading(16.0),
        title_medium: heading(14.0),
        title_small: text_style(13, 1.4).font_weight(FontWeight::MEDIUM),
        body_large: body(14.0).wrap(TextWrap::Word),
        body_medium: body(13.0).wrap(TextWrap::Word),
        body_small: body(12.0).wrap(TextWrap::Word),
        label_large: text_style(13, 1.4).font_weight(FontWeight::MEDIUM),
        label_medium: text_style(12, 1.4).font_weight(FontWeight::MEDIUM),
        label_small: text_style(11, 1.4).font_weight(FontWeight::MEDIUM),
    }
}

/// One line of interface text `pixels` tall, in the sans family and `tone`.
pub fn line(pixels: impl Into<f64>, tone: impl Into<Color>) -> TextStyle {
    text_style(pixels, 1.4).color(color(tone)).max_lines(1)
}

/// One line of interface text `pixels` tall at medium weight, in the sans
/// family and `tone`: titles and names that lead a row.
pub fn strong(pixels: impl Into<f64>, tone: impl Into<Color>) -> TextStyle {
    line(pixels, tone).font_weight(FontWeight::MEDIUM)
}

/// One line of text `pixels` tall in the mono family and `tone`: paths,
/// hashes, keys and counts.
pub fn code(pixels: impl Into<f64>, tone: impl Into<Color>) -> TextStyle {
    mono_style(pixels, 1.4).color(color(tone)).max_lines(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::set_scales;

    /// Installs this thread's text engine, which measuring text needs.
    fn fonts() {
        drop(rustle::Embedded::without_renderer(|| {}, || {}));
    }

    /// Line breaks, blank paragraphs and a trailing line break keep the byte ranges of the
    /// text when interface text is wrapped at spaces.
    #[test]
    fn wrapped_lines_keep_the_offsets_of_paragraphs() {
        fonts();
        let text = "é 🦀\n\nend\n";
        let line = |start, end, next| rustle::WrappedLine { start, end, next };
        assert_eq!(
            rustle::wrap_at_spaces(text, &text_style(14, 1.5), 1000.0),
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
            let lines = rustle::wrap_at_spaces(&text, &text_style(13, 1.5), width);
            let mut covered = 0;
            for line in &lines {
                assert!(text.is_char_boundary(line.start) && text.is_char_boundary(line.end));
                assert_eq!(line.start, covered, "at width {width}");
                covered = line.next.min(text.len());
            }
            assert_eq!(covered, text.len());
        }
    }

    /// Interface text follows the reader's text scale with its line height, on one unwrapped
    /// line; code keeps its size.
    #[test]
    fn interface_text_scales_and_code_does_not() {
        set_scales(1.0, 1.5);
        let interface = text_style(12, 1.5);
        let code = code_style(12, 1.5);
        set_scales(1.0, 1.0);
        assert_eq!(interface.font_size, 18.0);
        assert_eq!(interface.line_height, 27.0);
        assert_eq!(interface.wrap, TextWrap::None);
        assert_eq!(code.font_size, 12.0);
        assert_eq!(code.line_height, 18.0);
    }

    /// How often a skippable block that sets code and one beside it that reads nothing
    /// ambient ran, in a root that is never asked to compose again by hand.
    fn code_and_bystander_runs(change: impl FnOnce()) -> ((u32, u32), (u32, u32), bool) {
        use std::cell::Cell;
        use std::rc::Rc;
        let code_runs = Rc::new(Cell::new(0));
        let bystander_runs = Rc::new(Cell::new(0));
        let joined = Rc::new(Cell::new(true));
        let (code, bystander, seen) = (
            Rc::clone(&code_runs),
            Rc::clone(&bystander_runs),
            Rc::clone(&joined),
        );
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let (code, seen) = (Rc::clone(&code), Rc::clone(&seen));
                rustle::skippable((), move || {
                    code.set(code.get() + 1);
                    seen.set(code_style(13, 1.5).ligatures);
                });
                let bystander = Rc::clone(&bystander);
                rustle::skippable((), move || bystander.set(bystander.get() + 1));
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let before = (code_runs.get(), bystander_runs.get());
        change();
        ui.update();
        (
            before,
            (code_runs.get(), bystander_runs.get()),
            joined.get(),
        )
    }

    /// Switching ligatures off composes again the code that reads the switch, and nothing
    /// that does not, with no one asking for the whole tree to be composed again.
    #[test]
    fn switching_ligatures_composes_only_the_code_again() {
        let (before, after, joined) = code_and_bystander_runs(|| set_code_ligatures(false));
        assert_eq!(after.0, before.0 + 1, "the code was not composed again");
        assert_eq!(
            after.1, before.1,
            "a block reading no switch was composed again"
        );
        assert!(!joined, "the code kept its ligatures");
    }

    /// Choosing another code font composes again the code that is set in it.
    #[test]
    fn choosing_a_font_composes_the_code_again() {
        let (before, after, _) =
            code_and_bystander_runs(|| crate::theme::set_font_choice("", "Chosen Mono"));
        assert_eq!(after.0, before.0 + 1, "the code was not composed again");
        assert_eq!(
            after.1, before.1,
            "a block reading no font was composed again"
        );
    }

    /// Saying the same ligature switch again composes nothing.
    #[test]
    fn the_same_switch_composes_nothing() {
        let (before, after, joined) = code_and_bystander_runs(|| set_code_ligatures(true));
        assert_eq!(after, before);
        assert!(joined);
    }

    /// Code asked for at a weight code fonts lack is set in the interface family, and at the
    /// regular weight in the code family.
    #[test]
    fn code_keeps_its_family_only_at_regular_and_bold() {
        let regular = in_mono(text_style(13, 1.5));
        let medium = in_mono(text_style(13, 1.5).font_weight(FontWeight::MEDIUM));
        assert_eq!(regular.font_family, mono());
        assert_eq!(medium.font_family, sans());
    }
}

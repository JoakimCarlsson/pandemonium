//! The design tokens: colour roles resolved against the theme, corner radii,
//! the spacing step and the text scale, and the font families text is set in.
//!
//! Views never name colours: they name a [`Role`] or a [`Tone`] (a role, a
//! palette entry or an explicit value, optionally faded), and [`color`]
//! resolves it against the [`Theme`] the host provided with [`provide`] for
//! the frame being composed. Spacing counts steps of four logical pixels at
//! the density the host set with [`set_scales`]; interface text is scaled by
//! the reader's text size, code text is not.
//!
//! The colour and theme data ([`Role`], [`Tone`], [`Theme`], [`ThemeFamily`],
//! the shipped families and the families extensions bring) are the same data
//! the old interface used, re-exported here so ported code names them only
//! through the kit.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle::{CompositionLocal, FontFamily};

pub use pm_color::{Color as Tone, Role, Srgba};
pub use pm_theme::{
    Appearance, Fonts, Theme, ThemeFamily, default_family, families, family_from_colours,
    family_named, set_extension_families, shipped_families,
};

/// Logical pixels in one step of the spacing scale at the comfortable density.
pub const STEP_PX: f32 = 4.0;

thread_local! {
    /// The scale of a spacing step and the scale of interface text for what is composed on this thread.
    static SCALES: Cell<(f32, f32)> = const { Cell::new((1.0, 1.0)) };
    /// The font stacks last resolved, the families chosen in settings, and the families they resolved to, once fonts are known.
    static RESOLVED: RefCell<Option<Resolved>> = const { RefCell::new(None) };
    /// The interface and code families the reader chose in settings, by name; empty for the ones the app ships.
    static CHOSEN: RefCell<(String, String)> = const { RefCell::new((String::new(), String::new())) };
}

/// Font stacks with the families chosen in settings, and the installed families they resolved to: sans, then mono.
type Resolved = (Fonts, (String, String), FontFamily, FontFamily);

/// Sets interface text in the family `sans` and code in `mono`, each by name, before the families of the theme's stacks; an empty name keeps the stack.
pub fn set_font_choice(sans: &str, mono: &str) {
    CHOSEN.with(|chosen| *chosen.borrow_mut() = (sans.to_owned(), mono.to_owned()));
    fonts_changed();
}

/// Builds what follows on this thread with each spacing step `step` of its
/// comfortable room and interface text `text` times as big as it was drawn for.
pub fn set_scales(step: f32, text: f32) {
    SCALES.with(|scales| scales.set((step, text)));
}

/// The share of its comfortable room a spacing step takes on this thread.
pub fn step_scale() -> f32 {
    SCALES.with(|scales| scales.get().0)
}

/// How much bigger than it was drawn for interface text is on this thread.
pub fn text_scale() -> f32 {
    SCALES.with(|scales| scales.get().1)
}

/// `steps` steps of the spacing scale in logical pixels, at the density set with [`set_scales`].
///
/// `sp(4)` is sixteen pixels at the comfortable density. Old utilities that
/// counted steps (`p(4)`, `gap(2)`, `w(9)`, `size(4)`, `inset(1)`) take this.
pub fn sp(steps: impl Into<f64>) -> f32 {
    steps.into() as f32 * STEP_PX * step_scale()
}

/// How round a corner is, as named steps.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Radius {
    /// No rounding.
    None,
    /// Two pixels.
    Xs,
    /// Four pixels.
    #[default]
    Sm,
    /// Six pixels.
    Md,
    /// Eight pixels.
    Lg,
    /// Twelve pixels.
    Xl,
    /// Sixteen pixels.
    Xl2,
    /// Twenty-four pixels.
    Xl3,
    /// Thirty-two pixels.
    Xl4,
    /// Half the shorter side: a pill or a circle.
    Full,
}

impl Radius {
    /// The radius in logical pixels; [`Radius::Full`] is large enough to round any box into a pill.
    pub fn px(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Xs => 2.0,
            Self::Sm => 4.0,
            Self::Md => 6.0,
            Self::Lg => 8.0,
            Self::Xl => 12.0,
            Self::Xl2 => 16.0,
            Self::Xl3 => 24.0,
            Self::Xl4 => 32.0,
            Self::Full => 9999.0,
        }
    }
}

/// What the views of one frame are drawn with: the theme and the font stacks.
#[derive(Clone, Debug)]
pub struct Tokens {
    /// The colour of every role.
    pub theme: Rc<Theme>,
    /// Whether the theme is the light or the dark one of its family.
    pub appearance: Appearance,
}

impl PartialEq for Tokens {
    /// Tokens are equal when they hold the same theme, by identity or by value, in the same appearance.
    fn eq(&self, other: &Self) -> bool {
        self.appearance == other.appearance
            && (Rc::ptr_eq(&self.theme, &other.theme) || *self.theme == *other.theme)
    }
}

impl Tokens {
    /// The tokens of `family` in `appearance`.
    pub fn new(family: &ThemeFamily, appearance: Appearance) -> Self {
        Self {
            theme: Rc::new(family.variant(appearance).clone()),
            appearance,
        }
    }
}

impl Default for Tokens {
    /// The dark theme of the default family.
    fn default() -> Self {
        Self::new(&default_family(), Appearance::Dark)
    }
}

/// The tokens composables below a [`provide`] read.
static LOCAL_TOKENS: CompositionLocal<Tokens> = CompositionLocal::new(Tokens::default);

/// Composes `content` drawn with `tokens`.
pub fn provide(tokens: Tokens, content: impl FnOnce()) {
    LOCAL_TOKENS.provide(tokens, content);
}

/// The tokens the views being composed are drawn with.
pub fn tokens() -> Tokens {
    LOCAL_TOKENS.current()
}

/// The theme the views being composed are drawn with.
pub fn theme() -> Rc<Theme> {
    LOCAL_TOKENS.current().theme
}

/// `tone` resolved against the theme being composed with: a role takes the
/// theme's colour, an explicit value stays as it is.
pub fn color(tone: impl Into<Tone>) -> rustle::Color {
    color_in(&theme(), tone)
}

/// `tone` resolved against `theme`, for code that runs outside composition,
/// such as a drawing closure that captured the theme.
pub fn color_in(theme: &Theme, tone: impl Into<Tone>) -> rustle::Color {
    let [r, g, b, a] = tone.into().resolve(theme).to_bytes();
    rustle::Color::rgba(r, g, b, a)
}

/// The colour of `role` in the theme being composed with.
pub fn role(role: Role) -> rustle::Color {
    color(role)
}

/// The family interface text is set in: the first family of the theme's sans
/// stack that is installed, or the system's sans-serif family.
pub fn sans() -> FontFamily {
    families_for(theme().fonts).0
}

/// The family code is set in: the first family of the theme's mono stack that
/// is installed, or the system's monospaced family.
pub fn mono() -> FontFamily {
    families_for(theme().fonts).1
}

/// The installed families `fonts`, after the families chosen in settings, resolve to, sans then mono, remembered once fonts are known.
fn families_for(fonts: Fonts) -> (FontFamily, FontFamily) {
    let chosen = CHOSEN.with(|chosen| chosen.borrow().clone());
    if let Some((held, choice, sans, mono)) = RESOLVED.with(|resolved| resolved.borrow().clone())
        && held == fonts
        && choice == chosen
    {
        return (sans, mono);
    }
    let installed = rustle::font_families();
    let first = |choice: &str, stack: &[&str], fallback: FontFamily| {
        std::iter::once(choice)
            .filter(|name| !name.is_empty())
            .chain(stack.iter().copied())
            .find_map(|wanted| {
                installed
                    .iter()
                    .find(|name| name.eq_ignore_ascii_case(wanted))
            })
            .map_or(fallback, |name| FontFamily::named(name))
    };
    let sans = first(&chosen.0, fonts.sans, FontFamily::SansSerif);
    let mono = first(&chosen.1, fonts.mono, FontFamily::Monospace);
    if !installed.is_empty() {
        RESOLVED.with(|resolved| *resolved.borrow_mut() = Some((fonts, chosen, sans, mono)));
    }
    (sans, mono)
}

/// Forgets the families the font stacks resolved to, after fonts were installed or the reader chose others.
pub fn fonts_changed() {
    RESOLVED.with(|resolved| *resolved.borrow_mut() = None);
}

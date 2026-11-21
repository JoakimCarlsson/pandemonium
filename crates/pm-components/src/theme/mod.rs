//! The design tokens: colour roles resolved against the theme, corner radii,
//! the spacing step and the text scale, and the font families text is set in.
//!
//! Views never name colours: they name a [`Role`], which reads as the rustle
//! `Color` the [`Theme`] the host provided with [`provide`] gives it for the
//! frame being composed. Spacing counts steps of four logical pixels at
//! the density the host set with [`set_scales`]; interface text is scaled by
//! the reader's text size, code text is not.
//!
//! Colours are rustle's `Color`, with its maths; what lives here is values:
//! the roles ([`Role`]), one appearance's [`Theme`] with its [`Fonts`], the
//! [`ThemeFamily`] holding a dark and a light theme, the shipped families and
//! the families extensions bring, and their mapping onto rustle's theme.
//!
//! - [`role`]: the roles a view names instead of colours.
//! - [`scheme`], [`fonts`], [`family`]: a theme, its font stacks, a family of two.
//! - [`builtin`], [`extension`]: the shipped families and those extensions add.
//! - [`typography`]: the rustle text styles of interface text and code, the
//!   one-line styles views set their labels in, the rustle `Typography`
//!   widgets read, and the ligature switch.
//! - [`interaction`]: the theme's fills and rings for rustle's interaction support,
//!   and the fills of a selectable row.
//! - [`decoration`]: the app's shadows, and the outline and panel boxes are framed in.
//!
//! [`provide`] makes the theme the Compose way for a custom design system:
//! rustle's `theme` with [`color_scheme`], [`typography()`] and [`shapes`], the
//! scrollbar look as rustle's `LOCAL_SCROLLBAR_STYLE`, and one composition
//! local holding the [`Tokens`] rustle's scheme has no slot for.

pub mod builtin;
pub mod decoration;
pub mod extension;
pub mod family;
pub mod fonts;
pub mod interaction;
pub mod role;
pub mod scheme;
pub mod typography;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle::{Color, CompositionLocal, FontFamily};

pub use builtin::{default_family, families, family_named, shipped_families};
pub use decoration::{
    FLOATING_SHADOW, Frame, MEDIUM_SHADOW, SHADOW_SOFTNESS, ShadowLayers, box_shadow,
};
pub use extension::{family_from_colours, set_extension_families};
pub use family::{Appearance, ThemeFamily};
pub use fonts::Fonts;
pub use interaction::{FocusOutline, FocusRing, Focused, Indicated, InteractionTones, selectable};
pub use role::Role;
pub use scheme::Theme;
pub use typography::{
    code, code_cell, code_ligatures, code_style, fixed_text_style, font_families, font_key,
    in_mono, line, mono_style, set_code_ligatures, strong, text_style, typography,
};

/// Logical pixels in one step of the spacing scale at the comfortable density.
pub const STEP_PX: f32 = 4.0;

thread_local! {
    /// The scale of a spacing step and the scale of interface text for what is composed on this thread.
    static SCALES: Cell<(f32, f32)> = const { Cell::new((1.0, 1.0)) };
    /// The font stacks last resolved, the families chosen in settings, and the families they resolved to, once fonts are known.
    static RESOLVED: RefCell<Option<Resolved>> = const { RefCell::new(None) };
    /// The interface and code families the reader chose in settings, by name, empty for the ones the app ships, as snapshot state the composables setting text read.
    static CHOSEN: rustle::State<(String, String)> = rustle::mutable_state((String::new(), String::new()));
    /// The installed families as [`fonts_changed`] last found them, as snapshot state the composables setting text read; empty until it is first told.
    static INSTALLED: rustle::State<Vec<String>> = rustle::mutable_state(Vec::new());
}

/// Font stacks with the families chosen in settings, and the installed families they resolved to: sans, then mono.
type Resolved = (Fonts, (String, String), FontFamily, FontFamily);

/// Sets interface text in the family `sans` and code in `mono`, each by name, before the families of the theme's stacks; an empty name keeps the stack.
pub fn set_font_choice(sans: &str, mono: &str) {
    let choice = (sans.to_owned(), mono.to_owned());
    CHOSEN.with(|chosen| {
        if chosen.with(|held| *held != choice) {
            chosen.set(choice);
        }
    });
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

thread_local! {
    /// The tokens last provided on this thread, for code that runs outside composition.
    static LAST_TOKENS: RefCell<Option<Tokens>> = const { RefCell::new(None) };
}

/// Composes `content` drawn with `tokens`, which code running outside
/// composition on this thread, such as measuring text for the host, then uses too.
///
/// This is the app's theme, made as Compose's custom design systems are: rustle's
/// [`theme`](rustle::theme) carries the roles Compose has a slot for ([`color_scheme`]),
/// the type scale ([`typography`]) and the corner sizes ([`shapes`]), so rustle's
/// widgets and content colour follow it, and one composition local carries the
/// tokens it has no slot for (every [`Role`], the syntax colours, the font stacks),
/// read with [`tokens`], [`theme`] and [`color`].
pub fn provide(tokens: Tokens, content: impl FnOnce()) {
    LAST_TOKENS.with(|last| *last.borrow_mut() = Some(tokens.clone()));
    let scheme = color_scheme(&tokens.theme, tokens.appearance);
    let selection = selection_colors(&tokens.theme);
    LOCAL_TOKENS.provide(tokens, || {
        rustle::LOCAL_SCROLLBAR_STYLE.provide(scrollbar(), || {
            rustle::theme(scheme, typography(), shapes(), || {
                rustle::LOCAL_TEXT_SELECTION_COLORS.provide(selection, content);
            });
        });
    });
}

/// The colour of `role` in `theme`, its opacity scaled by `share`.
fn faded(theme: &Theme, role: Role, share: f32) -> Color {
    let color = theme.color(role);
    color.copy(color.alpha() * share)
}

/// The colours of selected text in `theme`, as a design system provides Compose's
/// `LocalTextSelectionColors`: the focus colour, translucent behind the text.
pub fn selection_colors(theme: &Theme) -> rustle::TextSelectionColors {
    rustle::TextSelectionColors {
        handle_color: theme.color(Role::Focused),
        background_color: faded(theme, Role::Focused, 0.45),
    }
}

/// The rustle colour scheme of `theme` in `appearance`: the roles laid on the
/// Compose roles that mean the same (accent as primary, surfaces and their
/// hover and pressed fills as the surface containers, borders as outlines,
/// danger as error), the rest kept from rustle's scheme of that appearance.
pub fn color_scheme(theme: &Theme, appearance: Appearance) -> rustle::ColorScheme {
    let role = |role: Role| theme.color(role);
    let base = match appearance {
        Appearance::Dark => rustle::dark_color_scheme(),
        Appearance::Light => rustle::light_color_scheme(),
    };
    rustle::ColorScheme {
        primary: role(Role::Accent),
        on_primary: role(Role::OnAccent),
        primary_container: role(Role::SurfaceSelected),
        on_primary_container: role(Role::Text),
        inverse_primary: role(Role::Accent),
        secondary: role(Role::Muted),
        on_secondary: role(Role::Background),
        secondary_container: role(Role::SurfaceHover),
        on_secondary_container: role(Role::Text),
        tertiary: role(Role::Success),
        on_tertiary: role(Role::OnSuccess),
        tertiary_container: role(Role::SurfaceActive),
        on_tertiary_container: role(Role::Text),
        background: role(Role::Background),
        on_background: role(Role::Text),
        surface: role(Role::Surface),
        on_surface: role(Role::Text),
        surface_variant: role(Role::SurfaceHover),
        on_surface_variant: role(Role::Muted),
        surface_tint: Color::TRANSPARENT,
        inverse_surface: role(Role::Text),
        inverse_on_surface: role(Role::Background),
        error: role(Role::Danger),
        on_error: role(Role::OnDanger),
        error_container: faded(theme, Role::Danger, 0.15),
        on_error_container: role(Role::Danger),
        outline: role(Role::Border),
        outline_variant: role(Role::Border),
        surface_bright: role(Role::SurfaceHover),
        surface_dim: role(Role::Background),
        surface_container_lowest: role(Role::Background),
        surface_container_low: role(Role::Sidebar),
        surface_container: role(Role::Surface),
        surface_container_high: role(Role::SurfaceHover),
        surface_container_highest: role(Role::SurfaceActive),
        ..base
    }
}

/// The corner sizes rustle's widgets read, from the named radii: menus and
/// fields four pixels, chips six, cards eight, then twelve, sixteen,
/// twenty-four and thirty-two, and pills fully round.
pub fn shapes() -> rustle::Shapes {
    let round = |radius: Radius| rustle::Shape::rounded(radius.px());
    rustle::Shapes {
        extra_small: round(Radius::Sm),
        small: round(Radius::Md),
        medium: round(Radius::Lg),
        large: round(Radius::Xl),
        large_increased: round(Radius::Xl2),
        extra_large: round(Radius::Xl3),
        extra_large_increased: round(Radius::Xl4),
        extra_extra_large: rustle::Shape::Circle,
    }
}

/// The tokens the views being composed are drawn with; outside composition,
/// the tokens last provided on this thread, or the default ones.
pub fn tokens() -> Tokens {
    if rustle::is_composing() {
        return LOCAL_TOKENS.current();
    }
    LAST_TOKENS.with(|last| last.borrow().clone().unwrap_or_default())
}

/// The theme the views being composed are drawn with; outside composition,
/// as [`tokens`] says.
pub fn theme() -> Rc<Theme> {
    tokens().theme
}

/// The colour `color` names: a [`Role`] read from the theme being composed
/// with, or a colour as it is.
pub fn color(color: impl Into<Color>) -> Color {
    color.into()
}

/// How the bar of a scrolling area looks in the theme being composed with: a
/// thin rounded thumb in the muted colour, a little stronger under the
/// pointer, always shown while the content is longer than the area, with a
/// strip along the edge wide enough to take hold of it.
pub fn scrollbar() -> rustle::ScrollbarStyle {
    let theme = theme();
    rustle::ScrollbarStyle {
        visibility: rustle::ScrollbarVisibility::Always,
        color: faded(&theme, Role::Muted, 0.55),
        active_color: faded(&theme, Role::Muted, 0.85),
        thickness: 4.0,
        hit_thickness: 14.0,
        min_length: 24.0,
        corner_radius: 2.0,
        hover_duration: std::time::Duration::ZERO,
    }
}

/// The colour of `role` in the theme being composed with.
pub fn role(role: Role) -> Color {
    theme().color(role)
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
    let chosen = CHOSEN.with(rustle::State::get);
    let announced = INSTALLED.with(rustle::State::get);
    if let Some((held, choice, sans, mono)) = RESOLVED.with(|resolved| resolved.borrow().clone())
        && held == fonts
        && choice == chosen
    {
        return (sans, mono);
    }
    let installed = if announced.is_empty() {
        rustle::font_families()
    } else {
        announced
    };
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

/// Forgets the families the font stacks resolved to, after fonts were installed or the reader chose others, and holds the families installed now, so the composables setting text are composed again when they changed.
pub fn fonts_changed() {
    RESOLVED.with(|resolved| *resolved.borrow_mut() = None);
    let installed = rustle::font_families();
    INSTALLED.with(|held| {
        if held.with(|families| *families != installed) {
            held.set(installed);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    /// What the root saw of the theme: rustle's scheme, its shapes and type scale, the
    /// scrollbar look, and the app's own colour of a role rustle has no slot for.
    type Seen = Option<(
        rustle::ColorScheme,
        rustle::Shapes,
        f32,
        rustle::ScrollbarStyle,
        Color,
    )>;

    /// Providing the tokens gives rustle's theme the app's roles, corners and type scale,
    /// the scrollbar its look, and the tokens' own local the roles rustle has no slot for.
    #[test]
    fn provide_themes_rustle_and_keeps_the_extra_tokens() {
        let tokens = Tokens::new(&default_family(), Appearance::Light);
        let seen: Rc<RefCell<Seen>> = Rc::default();
        let (held, kept) = (tokens.clone(), Rc::clone(&seen));
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let kept = Rc::clone(&kept);
                provide(held.clone(), move || {
                    *kept.borrow_mut() = Some((
                        rustle::color_scheme(),
                        rustle::shapes(),
                        rustle::typography().label_large.line_height_ratio(),
                        rustle::LOCAL_SCROLLBAR_STYLE.current(),
                        color(Role::SyntaxKeyword),
                    ));
                });
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let (scheme, corners, ratio, bar, keyword) = seen.borrow().expect("composed");
        let theme = &tokens.theme;
        assert_eq!(scheme.primary, theme.color(Role::Accent));
        assert_eq!(scheme.on_primary, theme.color(Role::OnAccent));
        assert_eq!(scheme.background, theme.color(Role::Background));
        assert_eq!(scheme.outline, theme.color(Role::Border));
        assert_eq!(scheme.error, theme.color(Role::Danger));
        assert_eq!(corners.small, rustle::Shape::rounded(Radius::Md.px()));
        assert!((ratio - 1.4).abs() < 1e-4);
        assert_eq!(bar.thickness, 4.0);
        assert_eq!(keyword, theme.color(Role::SyntaxKeyword));
    }
}

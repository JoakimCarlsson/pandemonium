//! The screen the window shows, and how it is drawn.

use pm_color::{Role, Roles, Srgba};
use pm_pages::{Screen, screen, settings_screen};
use pm_renderer::DrawList;
use pm_text::Shaper;
use pm_theme::{Fonts, Theme};
use pm_ui::{Input, Output};

/// A shaper set in the fonts the app ships.
pub fn shaper() -> Shaper {
    let mut shaper = Shaper::new();
    shaper.set_stacks(Fonts::DEFAULT.sans, Fonts::DEFAULT.mono);
    shaper
}

/// Sets `shaper` in the interface font `sans` and the code font `mono`, each by family name, falling back to the fonts the app ships; an empty name is the one the app ships.
pub fn set_fonts(shaper: &mut Shaper, sans: &str, mono: &str) {
    let stack = |chosen: &str, shipped: &[&'static str]| -> Vec<String> {
        std::iter::once(chosen)
            .filter(|name| !name.is_empty())
            .chain(shipped.iter().copied())
            .map(str::to_owned)
            .collect()
    };
    let sans = stack(sans, Fonts::DEFAULT.sans);
    let mono = stack(mono, Fonts::DEFAULT.mono);
    let sans: Vec<&str> = sans.iter().map(String::as_str).collect();
    let mono: Vec<&str> = mono.iter().map(String::as_str).collect();
    shaper.set_stacks(&sans, &mono);
}

/// Builds what follows on this thread as closely and with interface text as big as the window's settings in `state` say.
fn scale_for(state: &Screen) {
    let appearance = match state {
        Screen::Workspace(held) => held.window.appearance.clone(),
        Screen::Onboarding(_) => pm_config::AppearanceSettings::default(),
    };
    pm_style::set_scales(appearance.density.scale(), appearance.text_scale());
}

/// The colour the window is cleared to before anything is drawn.
pub fn ground(theme: &Theme) -> Srgba {
    theme.role(Role::Background)
}

/// Builds the screen for `state` and `input` and paints it into `list`,
/// replacing what the list held. Reports what was drawn and where.
pub fn paint(
    width: f32,
    height: f32,
    state: &Screen,
    input: &Input,
    theme: &Theme,
    shaper: &mut Shaper,
    list: &mut DrawList,
) -> Output {
    list.clear();
    scale_for(state);
    let root = screen(state, width, height, shaper);
    pm_ui::draw(&root, width, height, input, theme, shaper, list)
}

/// Builds what the settings window shows for `state` and `input` and paints
/// it into `list`, replacing what the list held. Reports what was drawn and
/// where; nothing is drawn while settings are closed.
pub fn paint_settings(
    width: f32,
    height: f32,
    state: &Screen,
    input: &Input,
    theme: &Theme,
    shaper: &mut Shaper,
    list: &mut DrawList,
) -> Output {
    list.clear();
    scale_for(state);
    match settings_screen(state, width) {
        Some(root) => pm_ui::draw(&root, width, height, input, theme, shaper, list),
        None => Output::default(),
    }
}

//! The screen the window shows, and how it is drawn.

use pm_color::{Role, Roles, Srgba};
use pm_pages::{Screen, picker_screen, screen, settings_screen};
use pm_renderer::DrawList;
use pm_text::Shaper;
use pm_theme::{Fonts, Theme};
use pm_ui::{Input, Node, Output};

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

/// Paints `root`, when there is one, into `list`, replacing what the list held, and reports what was drawn and where.
fn paint_window(
    root: Option<Node>,
    width: f32,
    height: f32,
    input: &Input,
    theme: &Theme,
    shaper: &mut Shaper,
    list: &mut DrawList,
) -> Output {
    match root {
        Some(root) => pm_ui::draw(&root, width, height, input, theme, shaper, list),
        None => Output::default(),
    }
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
    let root = settings_screen(state, width);
    paint_window(root, width, height, input, theme, shaper, list)
}

/// Builds what the folder picker's window shows for `state` and `input` and
/// paints it into `list`, replacing what the list held. Reports what was
/// drawn and where; nothing is drawn while the picker is closed.
pub fn paint_picker(
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
    let root = picker_screen(state);
    paint_window(root, width, height, input, theme, shaper, list)
}

/// The main screen retained until its model, viewport, clock second or fonts change.
#[derive(Default)]
pub struct Cached {
    /// The model version, logical viewport and clock second of the retained screen.
    key: Option<(u64, u32, u32, u64)>,
    /// The tree and layout of the last screen built.
    scene: Option<pm_ui::Scene>,
}

impl Cached {
    /// Drops geometry after a font change even when the model did not change.
    pub fn clear(&mut self) {
        self.key = None;
        self.scene = None;
    }

    /// Rebuilds on content, viewport or clock-second changes; otherwise only repaints interaction styles.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        version: u64,
        width: f32,
        height: f32,
        state: &Screen,
        input: &Input,
        theme: &Theme,
        shaper: &mut Shaper,
        list: &mut DrawList,
    ) -> Output {
        scale_for(state);
        let key = (
            version,
            width.to_bits(),
            height.to_bits(),
            (input.elapsed_ms / 1000.0) as u64,
        );
        if self.key != Some(key) {
            let root = screen(state, width, height, shaper);
            self.scene = Some(pm_ui::Scene::new(root, width, height, shaper));
            self.key = Some(key);
        }
        list.clear();
        self.scene
            .as_ref()
            .expect("screen built above")
            .paint(input, theme, shaper, list)
    }
}

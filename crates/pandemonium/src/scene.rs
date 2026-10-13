//! The screen the window shows, and how it is drawn.

use pm_color::{Role, Roles, Srgba};
use pm_components::{OnboardingState, onboarding};
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

/// The colour the window is cleared to before anything is drawn.
pub fn ground(theme: &Theme) -> Srgba {
    theme.role(Role::Background)
}

/// Builds the screen for `state` and `input` and paints it into `list`,
/// replacing what the list held. Reports what was drawn and where.
pub fn paint(
    width: f32,
    height: f32,
    state: &OnboardingState,
    input: &Input,
    theme: &Theme,
    shaper: &mut Shaper,
    list: &mut DrawList,
) -> Output {
    list.clear();
    let root = onboarding(state);
    pm_ui::draw(&root, width, height, input, theme, shaper, list)
}

//! Computed health shared by every worktree presentation.

mod store;

use pm_ui::Styled;

pub use store::Checks;

/// What checks and language server errors say about a worktree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Health {
    /// No completed checks or diagnostic source is available.
    #[default]
    Unknown,
    /// A check run is queued or executing.
    Running,
    /// Every check passed and there are no errors.
    Passing,
    /// A check failed or a language server reports an error.
    Failing,
}

impl Health {
    /// Draws running and failing badges, reserving their space for quiet states.
    pub fn badge<M: Clone + 'static>(self, theme: &pm_ui::Theme, detail: &str) -> pm_ui::Div<M> {
        let status = match self {
            Self::Unknown | Self::Passing => None,
            Self::Running => Some((pm_ui::IconName::LoadCircle, theme.colors.text_subtle)),
            Self::Failing => Some((pm_ui::IconName::Close, theme.colors.danger)),
        };
        pm_ui::icon_badge(status, detail).children(status.is_none().then(|| {
            pm_ui::h_flex()
                .w_px(pm_ui::IconSize::Small.pixels())
                .h_px(pm_ui::IconSize::Small.pixels())
        }))
    }
}

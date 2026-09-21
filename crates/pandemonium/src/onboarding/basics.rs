//! The settings themselves, in the order a first launch wants them.

use pm_ui::{Div, Styled, Theme, switch_field, v_flex};

use crate::onboarding::agent_section::agent_section;
use crate::onboarding::keymap_section::keymap_section;
use crate::onboarding::rule::rule;
use crate::onboarding::setup::{Message, Setup};
use crate::onboarding::theme_section::theme_section;

/// The settings themselves, in the order a first launch wants them.
pub(super) fn basics(theme: &Theme, setup: &Setup) -> Div<Message> {
    v_flex()
        .w_full()
        .gap_6()
        .child(theme_section(setup))
        .child(keymap_section(theme, setup))
        .child(agent_section(theme, setup))
        .child(switch_field(
            theme,
            Some("Vim Mode"),
            "Coming from vim? Modal editing is built in, not an extension",
            setup.vim_mode,
            Message::ToggleVimMode,
        ))
        .child(switch_field(
            theme,
            Some("Trust New Worktrees"),
            "Run language servers and tasks in a session's worktree without asking first",
            setup.trust_worktrees,
            Message::ToggleTrustWorktrees,
        ))
        .child(rule(theme))
        .child(switch_field(
            theme,
            None,
            "Help improve Pandemonium by sending anonymous usage data",
            setup.metrics,
            Message::ToggleMetrics,
        ))
        .child(switch_field(
            theme,
            None,
            "Send crash reports so the crashes you hit get fixed",
            setup.crash_reports,
            Message::ToggleCrashReports,
        ))
}

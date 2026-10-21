//! The hooks a session's Codex holds back until the reader trusts them, how the prompt about them shows, and what pressing its buttons does.

use pm_acp::Hook;
use pm_input::FocusId;

use crate::sample::Session;
use crate::state::{Outgoing, Page, WorkspaceState};

/// Lists the hooks held back, each with a button that trusts it.
pub const HOOKS_REVIEW: FocusId = FocusId(5300);

/// Trusts every hook held back.
pub const HOOKS_TRUST_ALL: FocusId = FocusId(5301);

/// Puts the prompt away and leaves the hooks untrusted.
pub const HOOKS_CONTINUE: FocusId = FocusId(5302);

/// The first of the buttons trusting one hook of the review; the rest follow it.
pub const HOOKS_TRUST_BASE: u64 = 5310;

/// How many hooks the review gives a button of their own.
pub const HOOKS_TRUST_ROWS: usize = 80;

/// How the prompt about a session's held-back hooks shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HooksPrompt {
    /// It does not show.
    #[default]
    Hidden,
    /// It says how many hooks are held back and asks what to do.
    Asking,
    /// It lists the hooks held back.
    Reviewing,
}

impl Session {
    /// Takes in the hooks the agent's Codex holds back now, asking about them again when they are not the ones it held back before.
    pub(crate) fn hooks_held(&mut self, hooks: Vec<Hook>) {
        if hooks.is_empty() {
            self.hooks_prompt = HooksPrompt::Hidden;
        } else if hooks != self.held_hooks && self.hooks_prompt != HooksPrompt::Reviewing {
            self.hooks_prompt = HooksPrompt::Asking;
        }
        self.held_hooks = hooks;
    }
}

impl WorkspaceState {
    /// What pressing the button `id` does to the prompt about the open session's held-back hooks, when it is one of the prompt's.
    pub fn press_hooks(&mut self, id: FocusId) -> bool {
        let Page::Session(open) = self.page else {
            return false;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return false;
        };
        let one =
            id.0.checked_sub(HOOKS_TRUST_BASE)
                .map(|offset| offset as usize)
                .filter(|index| *index < HOOKS_TRUST_ROWS.min(session.held_hooks.len()));
        let hooks = match (id, one) {
            (_, Some(index)) => vec![session.held_hooks[index].clone()],
            (HOOKS_TRUST_ALL, _) => {
                session.hooks_prompt = HooksPrompt::Hidden;
                session.held_hooks.clone()
            }
            (HOOKS_REVIEW, _) => {
                session.hooks_prompt = HooksPrompt::Reviewing;
                return true;
            }
            (HOOKS_CONTINUE, _) => {
                session.hooks_prompt = HooksPrompt::Hidden;
                return true;
            }
            _ => return false,
        };
        self.outbox.push(Outgoing::TrustHooks {
            session: open,
            hooks,
        });
        true
    }
}

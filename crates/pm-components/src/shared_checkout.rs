//! The notice above a session's prompt box that another session's agent is working in the same checkout, where the two would write over each other's files.
//!
//! Sessions share a checkout when their servers name the same canonical working tree for them on the same machine. Each worktree of a repository is a working tree of its own, so sessions in separate worktrees never share one, and chats, which work in scratch folders of their own, are left out.

use crate::kit::sp;
use rustle::Modifier;
use rustle::{Flex, flex_row};

use crate::kit::{IconName, Role, Styled};

use crate::parts::{clipped, glyph};
use crate::sample::{Session, Status};
use crate::state::WorkspaceState;

/// One session as the search for shared checkouts sees it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Occupant<'a> {
    /// The machine it is on, by destination, with nothing standing for this one.
    pub machine: Option<&'a str>,
    /// The canonical working tree its agent works in, or nothing when it is unknown or the session cannot share one, such as a chat or a closed session.
    pub tree: Option<&'a str>,
    /// Whether its agent is working on a turn now.
    pub working: bool,
}

/// For each of `occupants`, in their order, the positions of the others whose agents are working in the same working tree on the same machine.
///
/// An occupant without a working tree shares with none, and one whose agent is idle is named by none, though it still learns of the working ones it shares with.
pub fn overlaps(occupants: &[Occupant<'_>]) -> Vec<Vec<usize>> {
    occupants
        .iter()
        .enumerate()
        .map(|(index, occupant)| {
            let Some(tree) = occupant.tree else {
                return Vec::new();
            };
            occupants
                .iter()
                .enumerate()
                .filter(|(other, held)| {
                    *other != index
                        && held.working
                        && held.machine == occupant.machine
                        && held.tree == Some(tree)
                })
                .map(|(other, _)| other)
                .collect()
        })
        .collect()
}

/// Joins `titles` into a phrase: `A`, `A and B`, or `A, B and C`.
fn listed(titles: &[&str]) -> String {
    match titles {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What the notice says when the agents of the sessions called `titles` are working in the same checkout.
pub fn shared_words(titles: &[&str]) -> String {
    let verb = if titles.len() == 1 { "is" } else { "are" };
    format!(
        "{} {verb} also working in this folder; start one of them in a worktree so their changes stay apart",
        listed(titles)
    )
}

impl WorkspaceState {
    /// The machine and working tree `session` can share a checkout by, or nothing when it cannot share one: a chat, a closed session, the stand-in for a project's folder, one whose project is gone, or one whose working tree its server has not named.
    fn checkout_of<'a>(&self, session: &'a Session) -> Option<(Option<String>, &'a str)> {
        if session.is_chat() || session.closed || session.home {
            return None;
        }
        let machine = self.machine_of(session)?;
        Some((machine, session.tree.as_deref()?))
    }

    /// The titles of the other sessions whose agents are working now in the same checkout as `session`, in the order the sessions are held.
    pub fn sharing_checkout(&self, session: &Session) -> Vec<&str> {
        let Some(index) = self
            .sessions
            .iter()
            .position(|held| std::ptr::eq(held, session))
        else {
            return Vec::new();
        };
        let checkouts: Vec<_> = self
            .sessions
            .iter()
            .map(|held| self.checkout_of(held))
            .collect();
        let occupants: Vec<Occupant<'_>> = self
            .sessions
            .iter()
            .zip(&checkouts)
            .map(|(held, checkout)| Occupant {
                machine: checkout
                    .as_ref()
                    .and_then(|(machine, _)| machine.as_deref()),
                tree: checkout.as_ref().map(|(_, tree)| *tree),
                working: held.status == Status::Working,
            })
            .collect();
        overlaps(&occupants)
            .swap_remove(index)
            .into_iter()
            .map(|other| self.sessions[other].title.as_str())
            .collect()
    }
}

/// Emits the notice for `session` while the agents of other sessions are working in its checkout: one muted line with a fork before it, naming them and suggesting a worktree; nothing while none are.
#[track_caller]
pub fn notice(state: &WorkspaceState, session: &Session) {
    let titles = state.sharing_checkout(session);
    if titles.is_empty() {
        return;
    }
    let words = shared_words(&titles);
    flex_row(
        Modifier::new().pad_x(2.5).pad_y(0.5),
        Flex::new().items_center().gap(sp(1.5)),
        move || {
            glyph(IconName::GitFork, 3.5, Role::Warning);
            clipped(words, 12, Role::Muted);
        },
    );
}

#[cfg(test)]
/// Tests of finding sessions that share a checkout and of what the notice says.
mod tests {
    use super::*;

    /// An occupant on this machine in `tree`, working when `working` is set.
    fn at(tree: Option<&str>, working: bool) -> Occupant<'_> {
        Occupant {
            machine: None,
            tree,
            working,
        }
    }

    /// Two working agents in one working tree each learn of the other.
    #[test]
    fn working_agents_in_one_tree_overlap() {
        let found = overlaps(&[at(Some("/repo"), true), at(Some("/repo"), true)]);
        assert_eq!(found, vec![vec![1], vec![0]]);
    }

    /// An idle session learns of the working one in its tree before it is sent a prompt, but the working one is not told of the idle one.
    #[test]
    fn idle_sessions_learn_of_working_ones() {
        let found = overlaps(&[at(Some("/repo"), false), at(Some("/repo"), true)]);
        assert_eq!(found, vec![vec![1], Vec::<usize>::new()]);
    }

    /// Idle sessions in one tree do not overlap.
    #[test]
    fn idle_sessions_do_not_overlap() {
        let found = overlaps(&[at(Some("/repo"), false), at(Some("/repo"), false)]);
        assert_eq!(found, vec![Vec::<usize>::new(), Vec::new()]);
    }

    /// Separate worktrees of one repository are separate trees and do not overlap.
    #[test]
    fn separate_worktrees_do_not_overlap() {
        let found = overlaps(&[
            at(Some("/repo"), true),
            at(Some("/worktrees/repo-fix"), true),
        ]);
        assert_eq!(found, vec![Vec::<usize>::new(), Vec::new()]);
    }

    /// Sessions without a working tree, such as chats, overlap with nothing.
    #[test]
    fn sessions_without_a_tree_do_not_overlap() {
        let found = overlaps(&[at(None, true), at(None, true), at(Some("/repo"), true)]);
        assert_eq!(found, vec![Vec::<usize>::new(), Vec::new(), Vec::new()]);
    }

    /// The same path on two machines is two checkouts.
    #[test]
    fn other_machines_do_not_overlap() {
        let remote = Occupant {
            machine: Some("box"),
            tree: Some("/repo"),
            working: true,
        };
        let found = overlaps(&[at(Some("/repo"), true), remote]);
        assert_eq!(found, vec![Vec::<usize>::new(), Vec::new()]);
    }

    /// Every working session in the tree is named, in order.
    #[test]
    fn every_working_session_is_named() {
        let found = overlaps(&[
            at(Some("/repo"), false),
            at(Some("/repo"), true),
            at(Some("/elsewhere"), true),
            at(Some("/repo"), true),
        ]);
        assert_eq!(found[0], vec![1, 3]);
        assert_eq!(found[1], vec![3]);
        assert_eq!(found[3], vec![1]);
    }

    /// The notice names one session or several and suggests a worktree.
    #[test]
    fn words_name_the_sessions() {
        assert_eq!(
            shared_words(&["Fix login"]),
            "Fix login is also working in this folder; start one of them in a worktree so their changes stay apart"
        );
        assert!(shared_words(&["A", "B", "C"]).starts_with("A, B and C are also working"));
    }

    /// A session only learns of working sessions of the same project folder, and not of chats or of itself.
    #[test]
    fn the_state_names_working_sessions_in_the_same_checkout() {
        let mut state = WorkspaceState::sample();
        state.sessions.truncate(3);
        for (position, held) in state.sessions.iter_mut().enumerate() {
            held.project = Some(0);
            held.closed = false;
            held.home = false;
            held.title = format!("Task {position}");
            held.tree = Some("/repo".to_owned());
            held.status = Status::Idle;
        }
        state.sessions[1].status = Status::Working;
        state.sessions[2].status = Status::Working;
        state.sessions[2].project = None;
        assert_eq!(state.sharing_checkout(&state.sessions[0]), vec!["Task 1"]);
        assert!(state.sharing_checkout(&state.sessions[1]).is_empty());
        state.sessions[1].tree = Some("/worktrees/task".to_owned());
        assert!(state.sharing_checkout(&state.sessions[0]).is_empty());
    }
}

//! The list of branches under the branch of the repository the panel works on: to check one out, or to make a new one.

use crate::areas::GitEvent;
use crate::parts;
use rustle::{Alignment, Arrangement};

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::Modifier;

use crate::Input;
use crate::events::Interactive;

use crate::popover::{Popover, row};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::theme::{Role, sp};

/// How far down the panel the list starts, which is under the row of the branch, in logical pixels.
const TOP: f32 = 88.0;

/// How wide the list is, in logical pixels.
const WIDTH: f32 = 340.0;

/// Emits the list of branches of the repository the panel works on, hanging under the row it belongs to.
///
/// It is a focusable popup placed against the panel it is declared in, kept inside the window,
/// and answers to [`GitEvent::BranchBody`](crate::areas::GitEvent::BranchBody); Escape or a
/// press outside it closes it, a press on the button that opened it included, which the popup
/// takes so the button does not open it again.
#[track_caller]
pub fn branch_menu(state: &WorkspaceState) {
    let git = &state.git;
    let current = state.head_of(git.repository.as_deref()).unwrap_or_default();
    let current = current.as_str();
    let typed = git.filter.text();
    let typed = typed.trim();
    let matching = git.matching();
    crate::popups::in_box_at(
        (sp(3), TOP),
        crate::popups::Dismissal::focusable(WorkspaceState::without_branch_menu),
        || {
            Popover::new()
                .width(WIDTH)
                .modifier(Modifier::new().control(
                    Target::Git(GitEvent::BranchBody),
                    send(Event::Git(GitEvent::BranchBody)),
                ))
                .build_with(|| {
                    rustle::column_arranged(
                        Modifier::new().fill_max_width().padding_each(
                            theme::sp(1),
                            theme::sp(1),
                            theme::sp(1),
                            theme::sp(2),
                        ),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            Input::new(&git.filter, Target::Git(GitEvent::BranchFilter))
                                .placeholder("Search branches, or name a new one")
                                .size(Size::Sm)
                                .modifier(Modifier::new().fill_max_width())
                                .submits(Target::Git(GitEvent::BranchSubmit))
                                .build();
                        },
                    );
                    for (position, name) in matching.iter().enumerate() {
                        row(
                            name,
                            "",
                            name.as_str() == current,
                            Target::Git(GitEvent::BranchPick(position)),
                        );
                    }
                    if matching.is_empty() && typed.is_empty() {
                        parts::note(2.5, 1.5, "No branches.", 13, Role::Subtle);
                    }
                    if !typed.is_empty() && !git.exists() {
                        rustle::column_arranged(
                            Modifier::new().fill_max_width().padding_each(
                                0.0,
                                theme::sp(1),
                                0.0,
                                0.0,
                            ),
                            Arrangement::Start,
                            Alignment::Start,
                            || {
                                rustle::horizontal_divider(
                                    Modifier::new(),
                                    1.0,
                                    theme::color(Role::Border),
                                );
                                rustle::column_arranged(
                                    Modifier::new().fill_max_width().padding_each(
                                        0.0,
                                        theme::sp(1),
                                        0.0,
                                        0.0,
                                    ),
                                    Arrangement::Start,
                                    Alignment::Start,
                                    || {
                                        row(
                                            &format!("Create branch “{typed}”"),
                                            "From the commit this session is at",
                                            false,
                                            Target::Git(GitEvent::BranchCreate),
                                        );
                                    },
                                );
                            },
                        );
                    }
                });
        },
    );
}

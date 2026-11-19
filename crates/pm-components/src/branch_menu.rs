//! The list of branches under the branch of the repository the panel works on: to check one out, or to make a new one.

use crate::events::reduce;
use crate::theme;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged};

use crate::Input;
use crate::events::Interactive;
use crate::focus::FocusId;
use crate::git_controls::{BRANCH_BODY, BRANCH_CREATE, BRANCH_FILTER, BRANCH_PICK_BASE};
use crate::parts::line;
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
/// It is a popup placed against the panel it is declared in, kept inside the window, and answers
/// to [`BRANCH_BODY`]; a press elsewhere asks the app to close it.
#[track_caller]
pub fn branch_menu(state: &WorkspaceState) {
    let git = &state.git;
    let current = state.head_of(git.repository.as_deref()).unwrap_or_default();
    let current = current.as_str();
    let typed = git.filter.text();
    let typed = typed.trim();
    let matching = git.matching();
    crate::popups::in_box_at((sp(3), TOP), true, || {
        Popover::new()
            .width(WIDTH)
            .modifier(Modifier::new().control(BRANCH_BODY, reduce(BRANCH_BODY)))
            .build_with(|| {
                column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .padding_each(0.0, theme::sp(1), 0.0, 0.0)
                        .padding_each(theme::sp(1), 0.0, 0.0, 0.0)
                        .padding_each(0.0, 0.0, theme::sp(1), 0.0)
                        .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        Input::new(&git.filter, BRANCH_FILTER)
                            .placeholder("Search branches, or name a new one")
                            .size(Size::Sm)
                            .modifier(Modifier::new().fill_max_width())
                            .build();
                    },
                );
                for (position, name) in matching.iter().enumerate() {
                    row(
                        name,
                        "",
                        name.as_str() == current,
                        FocusId(BRANCH_PICK_BASE + position as u64),
                    );
                }
                if matching.is_empty() && typed.is_empty() {
                    column_arranged(
                        Modifier::new()
                            .padding_xy(theme::sp(2.5), 0.0)
                            .padding_xy(0.0, theme::sp(1.5)),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            line("No branches.", 13, 1.4, Role::Subtle);
                        },
                    );
                }
                if !typed.is_empty() && !git.exists() {
                    column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .padding_each(0.0, theme::sp(1), 0.0, 0.0)
                            .draw_behind(theme::edge(
                                theme::Side::Top,
                                1.0,
                                theme::color(Role::Border),
                            ))
                            .padding_each(0.0, 1.0, 0.0, 0.0)
                            .padding_each(0.0, theme::sp(1), 0.0, 0.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            row(
                                &format!("Create branch “{typed}”"),
                                "From the commit this session is at",
                                false,
                                BRANCH_CREATE,
                            );
                        },
                    );
                }
            });
    });
}

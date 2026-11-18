//! The list of branches under the branch of the repository the panel works on: to check one out, or to make a new one.

use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged};

use crate::Input;
use crate::git_controls::{BRANCH_BODY, BRANCH_CREATE, BRANCH_FILTER, BRANCH_PICK_BASE};
use crate::kit::{FocusId, Interactive, Role, Side, Styled, sp};
use crate::parts::line;
use crate::popover::{Popover, row};
use crate::size::Size;
use crate::state::WorkspaceState;

/// How far down the panel the list starts, which is under the row of the branch, in logical pixels.
const TOP: f32 = 88.0;

/// How wide the list is, in logical pixels.
const WIDTH: f32 = 340.0;

/// Emits the list of branches of the repository the panel works on, hanging under the row it belongs to.
///
/// It is placed in the panel's layer, kept inside the window, and answers to [`BRANCH_BODY`].
#[track_caller]
pub fn branch_menu(state: &WorkspaceState) {
    let git = &state.git;
    let current = state.head_of(git.repository.as_deref()).unwrap_or_default();
    let current = current.as_str();
    let typed = git.filter.text().trim();
    let matching = git.matching();
    Popover::new()
        .width(WIDTH)
        .modifier(
            Modifier::new()
                .at(sp(3), TOP)
                .z_index_unclipped(60)
                .keep_in_window()
                .target(BRANCH_BODY),
        )
        .build_with(|| {
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .pad_side(Side::Top, 1)
                    .pad_side(Side::Left, 1)
                    .pad_side(Side::Right, 1)
                    .pad_side(Side::Bottom, 2),
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
                    Modifier::new().pad_x(2.5).pad_y(1.5),
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
                        .pad_side(Side::Top, 1)
                        .stroke_side(Side::Top, 1.0, Role::Border)
                        .pad_side(Side::Top, 1),
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
}

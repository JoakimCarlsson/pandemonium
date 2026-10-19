//! The list of branches under the branch of the open session: to check one out, or to make a new one.

use pm_input::FocusId;
use pm_style::{
    Role, absolute, border_color, border_t, flex, flex_col, flex_none, left, mt, p, pb, px, py,
    top, z,
};
use pm_ui::{Node, node};

use crate::Input;
use crate::git_controls::{BRANCH_BODY, BRANCH_CREATE, BRANCH_FILTER, BRANCH_PICK_BASE};
use crate::parts::line;
use crate::popover::{Popover, row};
use crate::size::Size;
use crate::state::WorkspaceState;

/// How far down the panel the list starts, which is under the row of the branch, in logical pixels.
const TOP: f32 = 88.0;

/// How wide the list is, in logical pixels.
const WIDTH: f32 = 340.0;

/// The list of branches of the open session's repository, hanging under the row it belongs to.
pub fn branch_menu(state: &WorkspaceState, current: &str) -> Node {
    let git = &state.git;
    let typed = git.filter.text().trim();
    let matching = git.matching();
    let mut menu = Popover::new().width(WIDTH).child(
        node().styles([p(1), pb(2)]).child(
            Input::new(&git.filter, BRANCH_FILTER)
                .placeholder("Search branches, or name a new one")
                .size(Size::Sm)
                .build(),
        ),
    );
    menu = menu.children(matching.iter().enumerate().map(|(position, name)| {
        row(
            name,
            "",
            name.as_str() == current,
            FocusId(BRANCH_PICK_BASE + position as u64),
        )
    }));
    if matching.is_empty() && typed.is_empty() {
        menu = menu.child(node().styles([px(2.5), py(1.5)]).child(line(
            "No branches.",
            13,
            1.4,
            Role::Subtle,
        )));
    }
    if !typed.is_empty() && !git.exists() {
        menu = menu.child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    flex_none(),
                    border_t(),
                    border_color(Role::Border),
                    mt(1),
                    pm_style::pt(1),
                ])
                .child(row(
                    &format!("Create branch “{typed}”"),
                    "From the commit this session is at",
                    false,
                    BRANCH_CREATE,
                )),
        );
    }
    menu.build()
        .styles([absolute(), top(TOP / 4.0), left(3), z(60)])
        .focusable(BRANCH_BODY)
}

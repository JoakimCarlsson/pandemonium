//! The folder picker's window: the title bar that closes it over the picker, and the menu of its filter.

use pm_components::TitleBar;
use pm_components::folder_picker::FolderPicker;
use pm_components::title_bar::FRAMELESS;
use pm_style::{Role, bg, border, border_color, flex, flex_col, h_full, w_full};
use pm_ui::{Node, node};

/// What the folder picker's window shows: the title bar that closes it, the `picker` and the `menu` of its filter when one is open.
pub(crate) fn picker_window(picker: &FolderPicker, menu: Option<Node>) -> Node {
    node()
        .styles([flex(), flex_col(), w_full(), h_full(), bg(Role::Surface)])
        .styles(
            FRAMELESS
                .then(|| [border(), border_color(Role::Border)])
                .into_iter()
                .flatten(),
        )
        .child(TitleBar::closing().build())
        .child(picker.view())
        .children(menu)
}

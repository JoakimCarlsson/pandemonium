//! The menu that opens over a text field: its rows and the field it acts on.

use pm_input::FocusId;
use pm_text::Field;
use pm_ui::Node;

use crate::Menu;

/// Takes back the last change to the field the menu was opened on.
pub const MENU_UNDO: FocusId = FocusId(200);

/// Makes again the change that was taken back.
pub const MENU_REDO: FocusId = FocusId(201);

/// Copies the selection and deletes it.
pub const MENU_CUT: FocusId = FocusId(202);

/// Copies the selection.
pub const MENU_COPY: FocusId = FocusId(203);

/// Inserts what the clipboard holds.
pub const MENU_PASTE: FocusId = FocusId(204);

/// Selects the whole line.
pub const MENU_SELECT_ALL: FocusId = FocusId(205);

/// A menu open over a text field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenu {
    /// The field it acts on.
    pub field: FocusId,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
}

/// Whether `id` names a row of the menu.
pub fn is_menu_item(id: FocusId) -> bool {
    [
        MENU_UNDO,
        MENU_REDO,
        MENU_CUT,
        MENU_COPY,
        MENU_PASTE,
        MENU_SELECT_ALL,
    ]
    .contains(&id)
}

/// The menu opened at `open`, with each row enabled by what `field` can do.
pub fn field_menu(field: &Field, open: ContextMenu) -> Node {
    let editable = !field.is_read_only();
    Menu::new(open.at.0, open.at.1)
        .item(
            "Undo",
            Some("Ctrl+Z"),
            MENU_UNDO,
            !(editable && field.can_undo()),
        )
        .item(
            "Redo",
            Some("Ctrl+Shift+Z"),
            MENU_REDO,
            !(editable && field.can_redo()),
        )
        .separator()
        .item(
            "Cut",
            Some("Ctrl+X"),
            MENU_CUT,
            !(editable && field.can_copy()),
        )
        .item("Copy", Some("Ctrl+C"), MENU_COPY, !field.can_copy())
        .item("Paste", Some("Ctrl+V"), MENU_PASTE, !editable)
        .separator()
        .item(
            "Select all",
            Some("Ctrl+A"),
            MENU_SELECT_ALL,
            field.text().is_empty(),
        )
        .build()
}

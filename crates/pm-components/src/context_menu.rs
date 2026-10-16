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

/// The first of the rows of the menu over the file shown that act on code; each command's row follows it by the command's place.
const EDITOR_MENU_BASE: u64 = 7_402_000;

/// What a row of the menu over the file shown does, besides cutting, copying and pasting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorCommand {
    /// Goes to where the thing at the caret is defined.
    Definition,
    /// Goes to where the thing at the caret is declared.
    Declaration,
    /// Goes to where the type of the thing at the caret is defined.
    TypeDefinition,
    /// Goes to where the thing at the caret is implemented.
    Implementation,
    /// Lists every place the thing at the caret is used.
    References,
    /// Lists the functions that call the function at the caret.
    IncomingCalls,
    /// Lists the functions the function at the caret calls.
    OutgoingCalls,
    /// Gives the thing at the caret a new name across the project.
    Rename,
    /// Formats the file as its language says.
    Format,
    /// Lists the changes the language server offers at the caret.
    CodeActions,
    /// Copies the selection with the indentation its lines share taken off.
    CopyTrimmed,
    /// Shows the file in the file manager of this machine.
    Reveal,
    /// Starts a shell in the folder of the file.
    OpenTerminal,
    /// Copies the address of the line on the repository's host, at the commit checked out.
    Permalink,
    /// Lists the commits that changed the file.
    FileHistory,
}

impl EditorCommand {
    /// Every command, in the order of their rows' names.
    const ALL: [Self; 15] = [
        Self::Definition,
        Self::Declaration,
        Self::TypeDefinition,
        Self::Implementation,
        Self::References,
        Self::IncomingCalls,
        Self::OutgoingCalls,
        Self::Rename,
        Self::Format,
        Self::CodeActions,
        Self::CopyTrimmed,
        Self::Reveal,
        Self::OpenTerminal,
        Self::Permalink,
        Self::FileHistory,
    ];

    /// The name of the command's row.
    pub fn id(self) -> FocusId {
        let place = Self::ALL
            .iter()
            .position(|held| *held == self)
            .unwrap_or_default();
        FocusId(EDITOR_MENU_BASE + place as u64)
    }

    /// The command whose row `id` names.
    pub fn of(id: FocusId) -> Option<Self> {
        let place = id.0.checked_sub(EDITOR_MENU_BASE)?;
        Self::ALL.get(usize::try_from(place).ok()?).copied()
    }
}

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
        || EditorCommand::of(id).is_some()
}

/// How tall the menu over the file shown is, in logical pixels, to keep it inside the window.
pub const EDITOR_MENU_HEIGHT: f32 = 18.0 * 26.0 + 3.0 * 9.0 + 10.0;

/// The menu over the file shown, opened at `open`: the language server's ways around the code when `coding`, what the selection allows, and the file's place on this machine when it is on this machine, `local`.
pub fn editor_menu(field: &Field, open: ContextMenu, coding: bool, local: bool) -> Node {
    let editable = !field.is_read_only();
    let command = |menu: Menu, label: &str, keys: Option<&str>, which: EditorCommand, on: bool| {
        menu.item(label, keys, which.id(), !on)
    };
    let menu = Menu::new(open.at.0, open.at.1).width(66.0).row_height(6.5);
    let menu = command(
        menu,
        "Go to Definition",
        Some("F12"),
        EditorCommand::Definition,
        coding,
    );
    let menu = command(
        menu,
        "Go to Declaration",
        None,
        EditorCommand::Declaration,
        coding,
    );
    let menu = command(
        menu,
        "Go to Type Definition",
        Some("Ctrl+F12"),
        EditorCommand::TypeDefinition,
        coding,
    );
    let menu = command(
        menu,
        "Go to Implementation",
        Some("Shift+F12"),
        EditorCommand::Implementation,
        coding,
    );
    let menu = command(
        menu,
        "Find All References",
        Some("Alt+Shift+F12"),
        EditorCommand::References,
        coding,
    );
    let menu = command(
        menu,
        "Show Incoming Calls",
        Some("Alt+Shift+H"),
        EditorCommand::IncomingCalls,
        coding,
    );
    let menu = command(
        menu,
        "Show Outgoing Calls",
        None,
        EditorCommand::OutgoingCalls,
        coding,
    )
    .separator();
    let menu = command(
        menu,
        "Rename Symbol",
        Some("F2"),
        EditorCommand::Rename,
        coding && editable,
    );
    let menu = command(
        menu,
        "Format Buffer",
        Some("Alt+Shift+F"),
        EditorCommand::Format,
        coding && editable,
    );
    let menu = command(
        menu,
        "Show Code Actions",
        Some("Ctrl+."),
        EditorCommand::CodeActions,
        coding && editable,
    )
    .separator()
    .item(
        "Cut",
        Some("Ctrl+X"),
        MENU_CUT,
        !(editable && field.can_copy()),
    )
    .item("Copy", Some("Ctrl+C"), MENU_COPY, !field.can_copy());
    let menu = command(
        menu,
        "Copy and Trim",
        None,
        EditorCommand::CopyTrimmed,
        field.can_copy(),
    )
    .item("Paste", Some("Ctrl+V"), MENU_PASTE, !editable)
    .separator();
    let menu = command(
        menu,
        "Reveal in File Manager",
        Some("Ctrl+K R"),
        EditorCommand::Reveal,
        local,
    );
    let menu = command(
        menu,
        "Open in Terminal",
        None,
        EditorCommand::OpenTerminal,
        true,
    );
    let menu = command(
        menu,
        "Copy Permalink to Line",
        None,
        EditorCommand::Permalink,
        true,
    );
    command(
        menu,
        "View File History",
        None,
        EditorCommand::FileHistory,
        true,
    )
    .build()
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

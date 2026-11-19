//! The menu that opens over a text field: its rows and the field it acts on.

use crate::areas::MenuEvent;
use crate::target::Target;
use rustle::TextFieldState;

use crate::Menu;

/// What a row of the menu over the file shown does, besides cutting, copying and pasting.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
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
    /// Lists the types the type at the caret derives from.
    Supertypes,
    /// Lists the types that derive from the type at the caret.
    Subtypes,
    /// Gives the thing at the caret a new name across the project.
    Rename,
    /// Formats the file as its language says.
    Format,
    /// Formats the selection as its language says.
    FormatSelection,
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
    /// The name of the command's row.
    pub fn id(self) -> Target {
        Target::Menu(MenuEvent::EditorMenu(self))
    }

    /// The action of the keymap that does the same, when there is one.
    fn action(self) -> Option<pm_editor::Action> {
        use pm_editor::Action;
        Some(match self {
            Self::Definition => Action::GoToDefinition,
            Self::Declaration => Action::GoToDeclaration,
            Self::TypeDefinition => Action::GoToTypeDefinition,
            Self::Implementation => Action::GoToImplementation,
            Self::References => Action::FindReferences,
            Self::IncomingCalls => Action::IncomingCalls,
            Self::Rename => Action::Rename,
            Self::Format => Action::Format,
            Self::FormatSelection => Action::FormatSelection,
            Self::CodeActions => Action::CodeActions,
            Self::Reveal => Action::RevealInFileManager,
            _ => return None,
        })
    }

    /// The command whose row `id` names.
    pub fn of(id: Target) -> Option<Self> {
        match id {
            Target::Menu(MenuEvent::EditorMenu(command)) => Some(command),
            _ => None,
        }
    }
}

/// A menu open over a text field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenu {
    /// The field it acts on.
    pub field: Target,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
}

/// Whether `id` names a row of the menu.
pub fn is_menu_item(id: Target) -> bool {
    [
        Target::Menu(MenuEvent::MenuUndo),
        Target::Menu(MenuEvent::MenuRedo),
        Target::Menu(MenuEvent::MenuCut),
        Target::Menu(MenuEvent::MenuCopy),
        Target::Menu(MenuEvent::MenuPaste),
        Target::Menu(MenuEvent::MenuSelectAll),
    ]
    .contains(&id)
        || EditorCommand::of(id).is_some()
}

/// How tall the menu over the file shown is, in logical pixels, to keep it inside the window.
pub const EDITOR_MENU_HEIGHT: f32 = 18.0 * 26.0 + 3.0 * 9.0 + 10.0;

/// Emits the menu over the file shown, opened at `open`: the language server's ways around the code when `coding`, what the selection allows, and the file's place on this machine when it is on this machine, `local`; `keymap` names the keys of each command.
#[track_caller]
pub fn editor_menu(
    field: &pm_field::Field,
    open: ContextMenu,
    coding: bool,
    local: bool,
    keymap: &pm_editor::Keymap,
) {
    let editable = !field.is_read_only();
    let selected = field.selection().is_some();
    let command = |menu: Menu, label: &str, _: Option<&str>, which: EditorCommand, on: bool| {
        let keys = which.action().and_then(|action| keymap.label(action));
        menu.item(label, keys.as_deref(), which.id(), !on)
    };
    let menu = Menu::new(open.at.0, open.at.1)
        .width(66.0)
        .row_height(6.5)
        .on_pick(picked_in_field());
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
    );
    let menu = command(
        menu,
        "Show Supertypes",
        None,
        EditorCommand::Supertypes,
        coding,
    );
    let menu = command(menu, "Show Subtypes", None, EditorCommand::Subtypes, coding).separator();
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
        "Format Selection",
        Some("Ctrl+K Ctrl+F"),
        EditorCommand::FormatSelection,
        coding && editable && selected,
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
        Target::Menu(MenuEvent::MenuCut),
        !(editable && field.can_copy()),
    )
    .item(
        "Copy",
        Some("Ctrl+C"),
        Target::Menu(MenuEvent::MenuCopy),
        !field.can_copy(),
    );
    let menu = command(
        menu,
        "Copy and Trim",
        None,
        EditorCommand::CopyTrimmed,
        field.can_copy(),
    )
    .item(
        "Paste",
        Some("Ctrl+V"),
        Target::Menu(MenuEvent::MenuPaste),
        !editable,
    )
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
    .build();
}

/// What picking a row of a field's menu does: the app carries the row out on the field
/// the menu is open over.
fn picked_in_field() -> impl Fn(Target) + 'static {
    let operations = crate::events::operations();
    move |row| operations.field_menu_picked(row)
}

/// Emits the menu opened at `open`, with each row enabled by what `field` can
/// do; nothing is copied or cut from a `secure` field.
#[track_caller]
pub fn field_menu(field: &TextFieldState, open: ContextMenu, secure: bool) {
    let undo = field.undo_state();
    let copies = !secure && !field.selection().collapsed();
    Menu::new(open.at.0, open.at.1)
        .on_pick(picked_in_field())
        .item(
            "Undo",
            Some("Ctrl+Z"),
            Target::Menu(MenuEvent::MenuUndo),
            !undo.can_undo(),
        )
        .item(
            "Redo",
            Some("Ctrl+Shift+Z"),
            Target::Menu(MenuEvent::MenuRedo),
            !undo.can_redo(),
        )
        .separator()
        .item(
            "Cut",
            Some("Ctrl+X"),
            Target::Menu(MenuEvent::MenuCut),
            !copies,
        )
        .item(
            "Copy",
            Some("Ctrl+C"),
            Target::Menu(MenuEvent::MenuCopy),
            !copies,
        )
        .item(
            "Paste",
            Some("Ctrl+V"),
            Target::Menu(MenuEvent::MenuPaste),
            false,
        )
        .separator()
        .item(
            "Select all",
            Some("Ctrl+A"),
            Target::Menu(MenuEvent::MenuSelectAll),
            field.text().is_empty(),
        )
        .build();
}

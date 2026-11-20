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
    use MenuEvent::*;
    matches!(
        id,
        Target::Menu(MenuUndo | MenuRedo | MenuCut | MenuCopy | MenuPaste | MenuSelectAll)
    ) || EditorCommand::of(id).is_some()
}

/// How tall the menu over the file shown is, in logical pixels, to keep it inside the window.
pub const EDITOR_MENU_HEIGHT: f32 = 18.0 * 26.0 + 3.0 * 9.0 + 10.0;

/// Emits the menu over the file shown, opened at `open`: the language server's ways around the code when `coding`, what the selection allows, and the file's place on this machine when it is on this machine, `local`; `keymap` names the keys of each command.
pub fn editor_menu(
    field: &pm_field::Field,
    open: ContextMenu,
    coding: bool,
    local: bool,
    keymap: &pm_editor::Keymap,
) {
    let editable = !field.is_read_only();
    let selected = field.selection().is_some();
    let copies = field.can_copy();
    let edits = coding && editable;
    use EditorCommand as Do;
    let command = |label: &'static str, which: EditorCommand, on, rule| {
        let keys = which.action().and_then(|action| keymap.label(action));
        (label, keys, which.id(), on, rule)
    };
    let plain = |label: &'static str, keys: &str, event, on, rule| {
        (label, Some(keys.to_owned()), Target::Menu(event), on, rule)
    };
    Menu::new(open.at.0, open.at.1)
        .width(66.0)
        .row_height(6.5)
        .on_pick(picked_in_field())
        .rows([
            command("Go to Definition", Do::Definition, coding, false),
            command("Go to Declaration", Do::Declaration, coding, false),
            command("Go to Type Definition", Do::TypeDefinition, coding, false),
            command("Go to Implementation", Do::Implementation, coding, false),
            command("Find All References", Do::References, coding, false),
            command("Show Incoming Calls", Do::IncomingCalls, coding, false),
            command("Show Outgoing Calls", Do::OutgoingCalls, coding, false),
            command("Show Supertypes", Do::Supertypes, coding, false),
            command("Show Subtypes", Do::Subtypes, coding, true),
            command("Rename Symbol", Do::Rename, edits, false),
            command("Format Buffer", Do::Format, edits, false),
            command(
                "Format Selection",
                Do::FormatSelection,
                edits && selected,
                false,
            ),
            command("Show Code Actions", Do::CodeActions, edits, true),
            plain(
                "Cut",
                "Ctrl+X",
                MenuEvent::MenuCut,
                editable && copies,
                false,
            ),
            plain("Copy", "Ctrl+C", MenuEvent::MenuCopy, copies, false),
            command("Copy and Trim", Do::CopyTrimmed, copies, false),
            plain("Paste", "Ctrl+V", MenuEvent::MenuPaste, editable, true),
            command("Reveal in File Manager", Do::Reveal, local, false),
            command("Open in Terminal", Do::OpenTerminal, true, false),
            command("Copy Permalink to Line", Do::Permalink, true, false),
            command("View File History", Do::FileHistory, true, false),
        ])
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
    let rows = [
        (
            "Undo",
            "Ctrl+Z",
            MenuEvent::MenuUndo,
            undo.can_undo(),
            false,
        ),
        (
            "Redo",
            "Ctrl+Shift+Z",
            MenuEvent::MenuRedo,
            undo.can_redo(),
            true,
        ),
        ("Cut", "Ctrl+X", MenuEvent::MenuCut, copies, false),
        ("Copy", "Ctrl+C", MenuEvent::MenuCopy, copies, false),
        ("Paste", "Ctrl+V", MenuEvent::MenuPaste, true, true),
        (
            "Select all",
            "Ctrl+A",
            MenuEvent::MenuSelectAll,
            !field.text().is_empty(),
            false,
        ),
    ];
    Menu::new(open.at.0, open.at.1)
        .on_pick(picked_in_field())
        .rows(rows.map(|(label, keys, event, on, rule)| {
            (label, Some(keys.to_owned()), Target::Menu(event), on, rule)
        }))
        .build();
}

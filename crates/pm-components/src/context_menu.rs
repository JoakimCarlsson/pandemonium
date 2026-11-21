//! The menu that opens over a text field: its rows and the field it acts on.
//!
//! A text field hands rustle's [`TextContextMenu`] provided around it what it can do on its
//! text now, a [`TextManager`], as a Compose text field does; [`offered`] keeps the latest of
//! each field, and the field's menu offers those operations and runs them.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::areas::{ComposerEvent, MenuEvent};
use crate::target::Target;
use rustle::{TextContextMenu, TextFieldState, TextManager};

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

thread_local! {
    /// What each text field composed on this thread can do on its text, as it last said.
    static OFFERED: RefCell<BTreeMap<Target, TextManager>> = RefCell::default();
}

/// The text context menu for the field `id`, to provide around its rustle text field: it
/// keeps what the field says it can do on its text for the field's menu, and emits the field
/// as it is.
pub fn offered(id: Target) -> TextContextMenu {
    TextContextMenu::new(move |manager, content| {
        OFFERED.with(|offered| offered.borrow_mut().insert(id, manager));
        content();
    })
}

/// What the field `id` said, the last time it was composed, it can do on its text.
fn offered_by(id: Target) -> TextManager {
    OFFERED.with(|offered| offered.borrow().get(&id).cloned().unwrap_or_default())
}

/// Emits the menu over the file shown, opened at `open`: the language server's ways around the code when `coding`, what the selection allows, and the file's place on this machine when it is on this machine, `local`; `keymap` names the keys of each command.
pub fn editor_menu(
    field: &pm_editor::Field,
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

/// Emits the menu opened at `open`, with each row enabled by what `field` can do: its
/// history undoes and redoes, and the field's own operations cut, copy, paste and select
/// all, those it does not offer, such as copying from a secure field, doing nothing; the
/// prompt takes files and pictures pasted as attachments first.
#[track_caller]
pub fn field_menu(field: &TextFieldState, open: ContextMenu) {
    let undo = field.undo_state();
    let manager = offered_by(open.field);
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
        (
            "Cut",
            "Ctrl+X",
            MenuEvent::MenuCut,
            manager.cut.is_some(),
            false,
        ),
        (
            "Copy",
            "Ctrl+C",
            MenuEvent::MenuCopy,
            manager.copy.is_some(),
            false,
        ),
        ("Paste", "Ctrl+V", MenuEvent::MenuPaste, true, true),
        (
            "Select all",
            "Ctrl+A",
            MenuEvent::MenuSelectAll,
            !field.text().is_empty(),
            false,
        ),
    ];
    let operations = crate::events::operations();
    let picked = picked_in_field();
    Menu::new(open.at.0, open.at.1)
        .on_pick(move |row| {
            let run = |operation: &Option<std::rc::Rc<dyn Fn()>>| {
                if let Some(operation) = operation {
                    operation();
                }
            };
            match row {
                Target::Menu(MenuEvent::MenuUndo) => undo.undo(),
                Target::Menu(MenuEvent::MenuRedo) => undo.redo(),
                Target::Menu(MenuEvent::MenuCut) => run(&manager.cut),
                Target::Menu(MenuEvent::MenuCopy) => run(&manager.copy),
                Target::Menu(MenuEvent::MenuPaste)
                    if open.field == Target::Composer(ComposerEvent::Prompt)
                        && operations.paste_attachments() => {}
                Target::Menu(MenuEvent::MenuPaste) => run(&manager.paste),
                Target::Menu(MenuEvent::MenuSelectAll) => run(&manager.select_all),
                _ => {}
            }
            picked(row);
        })
        .rows(rows.map(|(label, keys, event, on, rule)| {
            (label, Some(keys.to_owned()), Target::Menu(event), on, rule)
        }))
        .build();
}

//! What a key pressed in the code editor asks for, before the document is
//! asked how to do it, and the keystroke vim reads for the same key.

use pm_text::Motion;
use pm_vim::{Key as VimKey, Keystroke};
use rustle::{Key, KeyEvent};

/// Something the code editor is asked to do by a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Types text at each caret.
    Type(String),
    /// Breaks the line, indented as the language says.
    Newline,
    /// Opens a line below the caret's, or above it when the flag is set.
    OpenLine(bool),
    /// Indents: the selected lines, or one level at the caret.
    Indent,
    /// Takes a level of indentation off the selected lines.
    Outdent,
    /// Deletes the selection or the character before the caret.
    Backspace,
    /// Deletes the selection or the word before the caret.
    BackspaceWord,
    /// Deletes the selection or the character after the caret.
    Delete,
    /// Deletes the selection or the word after the caret.
    DeleteWord,
    /// Moves the caret, extending the selection when the flag is set.
    Move(Motion, bool),
    /// Moves the caret to where its line's text starts, extending the selection when the flag is set.
    Home(bool),
    /// Moves the caret a page up, or down when the first flag is set, extending the selection when the second is.
    Page(bool, bool),
    /// Selects everything.
    SelectAll,
    /// Selects the caret's line, or the next one when a line is selected.
    SelectLine,
    /// Takes back the last change.
    Undo,
    /// Makes again the change taken back.
    Redo,
    /// Copies the selection.
    Copy,
    /// Copies the selection and deletes it.
    Cut,
    /// Pastes what the clipboard holds.
    Paste,
    /// Saves the file.
    Save,
    /// Opens finding.
    Find,
    /// Opens finding and replacing.
    Replace,
    /// Opens going to a line.
    GoToLine,
    /// Comments the selected lines out, or in again.
    ToggleComment,
    /// Moves the selected lines down, or up when the flag is not set.
    MoveLines(bool),
    /// Copies the selected lines below them, or above when the flag is not set.
    DuplicateLines(bool),
    /// Deletes the selected lines.
    DeleteLines,
    /// Joins the caret's line with the next.
    JoinLines,
    /// Moves the caret to the bracket matching the one beside it.
    MatchingBracket,
    /// Grows the selection to the syntax node around it.
    GrowSelection,
    /// Shrinks the selection back.
    ShrinkSelection,
    /// Closes what is open over the editor, or drops extra carets.
    Escape,
    /// Asks the language server for completions at the caret.
    Complete,
    /// Asks the language server where the name at the caret leads.
    Seek(pm_editor::Seek),
    /// Opens renaming the name at the caret.
    Rename,
    /// Asks the language server what changes it offers at the caret.
    CodeActions,
    /// Asks the language server what it says about the place at the caret.
    Hover,
    /// Asks the language server what the call the caret is in looks like.
    Signature,
    /// Asks the language server to sort the file's imports.
    OrganizeImports,
    /// Asks the language server to fix every problem of the file it can fix at once.
    FixAll,
    /// Asks the language server to format the file, or the selection.
    Format,
    /// Selects the next place the selection, or the word at the caret, comes again as well.
    AddNextOccurrence,
    /// Selects every place the selection, or the word at the caret, comes.
    SelectAllOccurrences,
    /// Adds a caret on the line below the carets, or above them when the flag is not set.
    AddCaret(bool),
    /// Folds away the block around the caret.
    Fold,
    /// Unfolds the blocks at the caret.
    Unfold,
    /// Folds away every outermost block.
    FoldAll,
    /// Unfolds every block.
    UnfoldAll,
}

/// What `event` asks the code editor for, or nothing for a key it leaves alone.
pub(crate) fn command(event: &KeyEvent) -> Option<Command> {
    if !event.pressed {
        return None;
    }
    let held = event.modifiers;
    let (ctrl, shift, alt) = (held.ctrl || held.meta, held.shift, held.alt);
    let letter = match &event.key {
        Key::Character(text) => text.chars().next().map(|first| first.to_ascii_lowercase()),
        _ => None,
    };
    if ctrl {
        return match (letter, &event.key) {
            (_, Key::ArrowUp) if alt => Some(Command::AddCaret(false)),
            (_, Key::ArrowDown) if alt => Some(Command::AddCaret(true)),
            (Some('[') | Some('{'), _) if alt => Some(Command::FoldAll),
            (Some(']') | Some('}'), _) if alt => Some(Command::UnfoldAll),
            (Some('[') | Some('{'), _) if shift => Some(Command::Fold),
            (Some(']') | Some('}'), _) if shift => Some(Command::Unfold),
            (Some('d'), _) if !shift => Some(Command::AddNextOccurrence),
            (Some('l'), _) if shift => Some(Command::SelectAllOccurrences),
            (Some(' '), _) if shift => Some(Command::Signature),
            (Some(' '), _) => Some(Command::Complete),
            (Some('.'), _) => Some(Command::CodeActions),
            (Some('k'), _) if !shift => Some(Command::Hover),
            (_, Key::F(12)) => Some(Command::Seek(pm_editor::Seek::Implementation)),
            (Some('s'), _) => Some(Command::Save),
            (Some('f'), _) => Some(Command::Find),
            (Some('h'), _) => Some(Command::Replace),
            (Some('g'), _) => Some(Command::GoToLine),
            (Some('z'), _) if shift => Some(Command::Redo),
            (Some('z'), _) => Some(Command::Undo),
            (Some('y'), _) => Some(Command::Redo),
            (Some('a'), _) => Some(Command::SelectAll),
            (Some('c'), _) => Some(Command::Copy),
            (Some('x'), _) => Some(Command::Cut),
            (Some('v'), _) => Some(Command::Paste),
            (Some('l'), _) => Some(Command::SelectLine),
            (Some('j'), _) => Some(Command::JoinLines),
            (Some('k'), _) if shift => Some(Command::DeleteLines),
            (Some('/'), _) => Some(Command::ToggleComment),
            (Some('\\'), _) | (Some('|'), _) if shift => Some(Command::MatchingBracket),
            (_, Key::Enter) => Some(Command::OpenLine(shift)),
            (_, Key::Backspace) => Some(Command::BackspaceWord),
            (_, Key::Delete) => Some(Command::DeleteWord),
            (_, Key::ArrowLeft) => Some(Command::Move(Motion::WordLeft, shift)),
            (_, Key::ArrowRight) => Some(Command::Move(Motion::WordRight, shift)),
            (_, Key::Home) => Some(Command::Move(Motion::Home, shift)),
            (_, Key::End) => Some(Command::Move(Motion::End, shift)),
            _ => None,
        };
    }
    if alt {
        return match (&event.key, shift) {
            (Key::Character(text), true) if text.eq_ignore_ascii_case("o") => {
                Some(Command::OrganizeImports)
            }
            (Key::Character(text), true) if text.eq_ignore_ascii_case("f") => Some(Command::Format),
            (Key::Character(text), true) if text == "." || text == ">" => Some(Command::FixAll),
            (Key::ArrowUp, false) => Some(Command::MoveLines(false)),
            (Key::ArrowDown, false) => Some(Command::MoveLines(true)),
            (Key::ArrowUp, true) => Some(Command::DuplicateLines(false)),
            (Key::ArrowDown, true) => Some(Command::DuplicateLines(true)),
            (Key::ArrowRight, true) => Some(Command::GrowSelection),
            (Key::ArrowLeft, true) => Some(Command::ShrinkSelection),
            _ => None,
        };
    }
    match &event.key {
        Key::F(12) if shift => Some(Command::Seek(pm_editor::Seek::References)),
        Key::F(12) => Some(Command::Seek(pm_editor::Seek::Definition)),
        Key::F(2) => Some(Command::Rename),
        Key::Enter => Some(Command::Newline),
        Key::Tab if shift => Some(Command::Outdent),
        Key::Tab => Some(Command::Indent),
        Key::Backspace => Some(Command::Backspace),
        Key::Delete => Some(Command::Delete),
        Key::Escape => Some(Command::Escape),
        Key::ArrowLeft => Some(Command::Move(Motion::Left, shift)),
        Key::ArrowRight => Some(Command::Move(Motion::Right, shift)),
        Key::ArrowUp => Some(Command::Move(Motion::LineUp, shift)),
        Key::ArrowDown => Some(Command::Move(Motion::LineDown, shift)),
        Key::Home => Some(Command::Home(shift)),
        Key::End => Some(Command::Move(Motion::LineEnd, shift)),
        Key::PageUp => Some(Command::Page(false, shift)),
        Key::PageDown => Some(Command::Page(true, shift)),
        _ => typed(event).map(Command::Type),
    }
}

/// The text `event` types, when it types printable text.
fn typed(event: &KeyEvent) -> Option<String> {
    let text = match (&event.text, &event.key) {
        (Some(text), _) if !text.is_empty() => text.clone(),
        (None, Key::Character(text)) => text.clone(),
        _ => return None,
    };
    (!text.chars().any(char::is_control)).then_some(text)
}

/// The keystroke vim reads for `event`, or nothing for a key it has no name for.
pub(crate) fn keystroke(event: &KeyEvent) -> Option<Keystroke> {
    if !event.pressed {
        return None;
    }
    let ctrl = event.modifiers.ctrl;
    let key = match &event.key {
        Key::Escape => VimKey::Escape,
        Key::Enter => VimKey::Enter,
        Key::Backspace => VimKey::Backspace,
        Key::Delete => VimKey::Delete,
        Key::Insert => VimKey::Insert,
        Key::Tab => VimKey::Tab,
        Key::ArrowLeft => VimKey::Left,
        Key::ArrowRight => VimKey::Right,
        Key::ArrowUp => VimKey::Up,
        Key::ArrowDown => VimKey::Down,
        Key::Home => VimKey::Home,
        Key::End => VimKey::End,
        Key::PageUp => VimKey::PageUp,
        Key::PageDown => VimKey::PageDown,
        Key::Character(unmodified) if ctrl => {
            VimKey::Char(unmodified.chars().next()?.to_ascii_lowercase())
        }
        _ => {
            let text = typed(event)?;
            let mut characters = text.chars();
            let first = characters.next()?;
            if characters.next().is_some() {
                return None;
            }
            VimKey::Char(first)
        }
    };
    let named = !matches!(key, VimKey::Char(_));
    Some(Keystroke {
        key,
        ctrl,
        shift: named && event.modifiers.shift,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustle::Modifiers;

    /// A press of `key` with `modifiers` held, typing nothing of its own.
    fn press(key: Key, modifiers: Modifiers) -> KeyEvent {
        KeyEvent::press(key, modifiers)
    }

    /// Plain keys type, move and break lines; chords save and find.
    #[test]
    fn keys_map_to_commands() {
        let plain = Modifiers::default();
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        assert_eq!(command(&press(Key::Enter, plain)), Some(Command::Newline));
        assert_eq!(
            command(&press(Key::Character("s".into()), ctrl)),
            Some(Command::Save)
        );
        assert_eq!(
            command(&press(Key::ArrowDown, plain)),
            Some(Command::Move(Motion::LineDown, false))
        );
        assert_eq!(
            command(&KeyEvent::typed("é", plain)),
            Some(Command::Type("é".into()))
        );
        assert_eq!(
            command(&press(Key::Character("d".into()), ctrl)),
            Some(Command::AddNextOccurrence)
        );
    }

    /// Vim reads characters and named keys.
    #[test]
    fn keys_map_to_keystrokes() {
        let plain = Modifiers::default();
        let stroke = keystroke(&KeyEvent::typed("j", plain)).unwrap();
        assert_eq!(stroke.key, VimKey::Char('j'));
        let escape = keystroke(&press(Key::Escape, plain)).unwrap();
        assert_eq!(escape.key, VimKey::Escape);
    }
}

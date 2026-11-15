//! The menus of the composer: the commands a slash narrows, the files an `@` mentions, the values of a
//! toolbar setting, and the text field's Cut, Copy, Paste and Select all;
//! and pasting files and pictures from the clipboard as attachments.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{TextFieldState, clipboard};

use crate::composer::{ComposerCallbacks, ComposerChoice, ComposerCommand};
use crate::menu::Menu;
use crate::size::Size;

/// How many commands the slash menu lists at most.
const COMMANDS_SHOWN: usize = 8;

/// How far a menu stays clear of what opened it, in logical pixels.
const GAP: f32 = 4.0;

/// How tall a toolbar button is, in logical pixels.
const BUTTON_HEIGHT: f32 = 22.0;

/// The menu of the commands the slash being typed narrows, opening upward over the composer.
pub(crate) struct CommandMenu {
    /// The commands listed, in order.
    pub(crate) commands: Vec<ComposerCommand>,
    /// What picking a command does with its name.
    pub(crate) on_pick: Rc<dyn Fn(String)>,
    /// What closing the menu without picking does.
    pub(crate) on_dismiss: Rc<dyn Fn()>,
}

impl CommandMenu {
    /// The commands among `commands` the slash at the start of `text` narrows, while no space follows its word.
    pub(crate) fn matching(commands: &[ComposerCommand], text: &str) -> Vec<ComposerCommand> {
        let Some(word) = text.strip_prefix('/') else {
            return Vec::new();
        };
        if word.contains(char::is_whitespace) {
            return Vec::new();
        }
        let word = word.to_lowercase();
        commands
            .iter()
            .filter(|command| command.name.to_lowercase().starts_with(&word))
            .take(COMMANDS_SHOWN)
            .cloned()
            .collect()
    }

    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let mut menu = Menu::new()
            .heading("Commands")
            .size(Size::Xs)
            .width(360.0)
            .above(true)
            .offset(0.0, GAP + 12.0);
        for command in self.commands {
            let on_pick = self.on_pick.clone();
            let name = command.name.clone();
            menu = menu
                .item(format!("/{}", command.name), move || on_pick(name.clone()))
                .hint(command.description);
        }
        let dismiss = self.on_dismiss;
        menu.on_dismiss(move || dismiss()).show();
    }
}

/// How many files the mention menu lists at most.
const MENTIONS_SHOWN: usize = 8;

/// The word being typed at the end of `text`, when it starts with an `@`, without the `@` and in lower case.
pub(crate) fn mention_query(text: &str) -> Option<String> {
    let word = text.rsplit(char::is_whitespace).next()?;
    word.strip_prefix('@').map(str::to_lowercase)
}

/// `text` with the `@` word at its end replaced by a mention of `path` and a space.
pub(crate) fn with_mention(text: &str, path: &str) -> String {
    let start = text
        .rfind(char::is_whitespace)
        .map_or(0, |at| at + text[at..].chars().next().map_or(1, char::len_utf8));
    format!("{}@{path} ", &text[..start])
}

/// The menu of the files the `@` being typed narrows, opening upward over the composer.
pub(crate) struct MentionMenu {
    /// The paths listed, in order.
    pub(crate) paths: Vec<String>,
    /// What picking a file does with its path.
    pub(crate) on_pick: Rc<dyn Fn(String)>,
    /// What closing the menu without picking does.
    pub(crate) on_dismiss: Rc<dyn Fn()>,
}

impl MentionMenu {
    /// The paths among `paths` holding `query`, those whose name starts with it first.
    pub(crate) fn matching(paths: &[String], query: &str) -> Vec<String> {
        let name = |path: &str| {
            path.rsplit('/')
                .next()
                .unwrap_or(path)
                .to_lowercase()
                .starts_with(query)
        };
        let mut found: Vec<&String> = paths
            .iter()
            .filter(|path| path.to_lowercase().contains(query))
            .collect();
        found.sort_by_key(|path| !name(path));
        found.into_iter().take(MENTIONS_SHOWN).cloned().collect()
    }

    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let mut menu = Menu::new()
            .heading("Files")
            .size(Size::Xs)
            .width(360.0)
            .above(true)
            .offset(0.0, GAP + 12.0);
        for path in self.paths {
            let on_pick = self.on_pick.clone();
            let picked = path.clone();
            menu = menu.icon_item(phosphor::FILE, path, move || on_pick(picked.clone()));
        }
        let dismiss = self.on_dismiss;
        menu.on_dismiss(move || dismiss()).show();
    }
}

/// The menu of a toolbar setting's values, opening upward over its button.
pub(crate) struct SettingMenu {
    /// The setting.
    pub(crate) choice: ComposerChoice,
    /// What picking a value does with the setting's id and the value.
    pub(crate) on_pick: Option<Rc<dyn Fn(String, String)>>,
    /// What closing the menu does.
    pub(crate) on_dismiss: Rc<dyn Fn()>,
}

impl SettingMenu {
    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let mut menu = Menu::new()
            .heading(self.choice.heading.clone())
            .size(Size::Xs)
            .width(260.0)
            .above(true)
            .offset(0.0, GAP + BUTTON_HEIGHT);
        for pick in self.choice.picks {
            let on_pick = self.on_pick.clone();
            let id = self.choice.id.clone();
            let value = pick.value.clone();
            let select = move || {
                if let Some(on_pick) = &on_pick {
                    on_pick(id.clone(), value.clone());
                }
            };
            menu = if pick.value == self.choice.current {
                menu.icon_item(phosphor::CHECK, pick.name, select)
            } else {
                menu.item(pick.name, select)
            };
            if !pick.description.is_empty() {
                menu = menu.hint(pick.description);
            }
        }
        let dismiss = self.on_dismiss;
        menu.on_dismiss(move || dismiss()).show();
    }
}

/// The text field's menu, opening upward from where the secondary button was pressed.
pub(crate) struct FieldMenu {
    /// The text field it acts on.
    pub(crate) state: TextFieldState,
    /// What pasting files or a picture calls.
    pub(crate) callbacks: ComposerCallbacks,
    /// Where it opens inside the text area, across and down.
    pub(crate) at: (f32, f32),
    /// What closing the menu does.
    pub(crate) on_dismiss: Rc<dyn Fn()>,
}

impl FieldMenu {
    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let selected = self
            .state
            .value()
            .selected_text()
            .map(str::to_owned)
            .unwrap_or_default();
        let cut_state = self.state.clone();
        let cut_text = selected.clone();
        let paste_state = self.state.clone();
        let all_state = self.state.clone();
        let callbacks = self.callbacks.clone();
        let dismiss = self.on_dismiss.clone();
        stack_with(Modifier::new().offset(self.at.0, self.at.1), move || {
            let mut menu = Menu::new().size(Size::Xs).width(180.0).above(true);
            menu = if selected.is_empty() {
                menu.disabled_item(phosphor::SCISSORS, "Cut")
                    .disabled_item(phosphor::COPY, "Copy")
            } else {
                menu.icon_item(phosphor::SCISSORS, "Cut", move || {
                    copy_text(&cut_text);
                    cut_state.replace_selection("");
                })
                .icon_item(phosphor::COPY, "Copy", move || copy_text(&selected))
            };
            menu.icon_item(phosphor::CLIPBOARD_TEXT, "Paste", move || {
                if paste_attachments(&callbacks) {
                    return;
                }
                if let Some(text) = clipboard().and_then(|clipboard| clipboard.text()) {
                    paste_state.replace_selection(&text);
                }
            })
            .hint("Ctrl+V")
            .separator()
            .icon_item(phosphor::SELECTION_ALL, "Select all", move || {
                all_state.select_all()
            })
            .hint("Ctrl+A")
            .on_dismiss(move || dismiss())
            .show();
        });
    }
}

/// Puts `text` on the clipboard.
fn copy_text(text: &str) {
    if let Some(clipboard) = clipboard() {
        clipboard.set_text(text);
    }
}

/// Attaches the files or the picture on the clipboard through `callbacks`,
/// and reports whether there were any; text is left for the field to paste.
pub(crate) fn paste_attachments(callbacks: &ComposerCallbacks) -> bool {
    let Some(clipboard) = clipboard() else {
        return false;
    };
    if let Some(on_attach) = &callbacks.on_attach {
        let files = clipboard.files();
        if !files.is_empty() {
            on_attach(
                files
                    .iter()
                    .map(|file| file.to_string_lossy().into_owned())
                    .collect(),
            );
            return true;
        }
    }
    if let Some(on_picture) = &callbacks.on_picture
        && let Some(picture) = clipboard.image()
    {
        on_picture(picture.width, picture.height, picture.rgba);
        return true;
    }
    false
}

#[cfg(test)]
/// Tests for mentioning files.
mod tests {
    use super::*;

    /// The word after an `@` at the end of the text is the query, and picking replaces it.
    #[test]
    fn a_mention_replaces_its_word() {
        assert_eq!(mention_query("look at @Mai").as_deref(), Some("mai"));
        assert_eq!(mention_query("look at @main.rs now"), None);
        assert_eq!(with_mention("look at @ma", "src/main.rs"), "look at @src/main.rs ");
        assert_eq!(with_mention("@", "a.rs"), "@a.rs ");
        let paths = ["src/lib/main.rs".to_owned(), "main.rs".to_owned(), "x.rs".to_owned()];
        assert_eq!(MentionMenu::matching(&paths, "main"), ["main.rs", "src/lib/main.rs"]);
    }
}

//! The menus of the composer: the commands a slash narrows, the files an `@` mentions, the values of a
//! toolbar setting, and the text field's Cut, Copy, Paste and Select all;
//! and pasting files and pictures from the clipboard as attachments.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{TextFieldState, clipboard};

use crate::composer::{ComposerCallbacks, ComposerChoice, ComposerCommand, ComposerPick};
use crate::menu::{Menu, MenuVariant};
use crate::popover::Popover;
use crate::size::Size;
use crate::text_input::TextInput;
use crate::text_menu::TextMenu;

/// How many commands the slash menu shows at once; the arrow keys step through the rest.
const COMMANDS_SHOWN: usize = 8;

/// How many values a toolbar setting has before its menu offers a filter.
const FILTER_FROM: usize = 8;

/// How tall the filtered values of a toolbar setting grow before they scroll, in logical pixels.
const FILTERED_HEIGHT: f32 = 320.0;

/// How far a menu stays clear of what opened it, in logical pixels.
const GAP: f32 = 4.0;

/// How tall a toolbar button is, in logical pixels.
const BUTTON_HEIGHT: f32 = 22.0;

/// The menu of the commands the slash being typed narrows, opening upward over the composer.
pub(crate) struct CommandMenu {
    /// The commands listed, in order.
    pub(crate) commands: Vec<ComposerCommand>,
    /// The command the arrow keys point at, which Enter picks.
    pub(crate) highlighted: usize,
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
            .cloned()
            .collect()
    }

    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let mut menu = Menu::new()
            .keyboard(false)
            .heading("Commands")
            .size(Size::Xs)
            .width(360.0)
            .above(true)
            .offset(0.0, GAP + 12.0);
        let (first, lit) = window(self.commands.len(), self.highlighted, COMMANDS_SHOWN);
        menu = menu.highlighted(Some(lit));
        for command in self.commands.into_iter().skip(first).take(COMMANDS_SHOWN) {
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

/// How many files the mention menu shows at once.
const MENTIONS_SHOWN: usize = 8;

/// How many files the mention menu can step through.
const MENTIONS_LISTED: usize = 50;

/// The first of `count` entries a list showing `shown` at once starts at so that the entry at `highlighted` is in view, and that entry's place among those shown.
fn window(count: usize, highlighted: usize, shown: usize) -> (usize, usize) {
    let first = highlighted
        .saturating_sub(shown.saturating_sub(1))
        .min(count.saturating_sub(shown));
    (first, highlighted - first)
}

/// The word being typed at the end of `text`, when it starts with an `@`, without the `@` and in lower case.
pub(crate) fn mention_query(text: &str) -> Option<String> {
    let word = text.rsplit(char::is_whitespace).next()?;
    word.strip_prefix('@').map(str::to_lowercase)
}

/// `text` with the `@` word at its end replaced by a mention of `path` and a space.
pub(crate) fn with_mention(text: &str, path: &str) -> String {
    let start = text.rfind(char::is_whitespace).map_or(0, |at| {
        at + text[at..].chars().next().map_or(1, char::len_utf8)
    });
    format!("{}@{path} ", &text[..start])
}

/// The menu of the files the `@` being typed narrows, opening upward over the composer.
pub(crate) struct MentionMenu {
    /// The paths listed, in order.
    pub(crate) paths: Vec<String>,
    /// The path the arrow keys point at, which Enter picks.
    pub(crate) highlighted: usize,
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
        found.into_iter().take(MENTIONS_LISTED).cloned().collect()
    }

    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let mut menu = Menu::new()
            .keyboard(false)
            .heading("Files")
            .size(Size::Xs)
            .width(360.0)
            .above(true)
            .offset(0.0, GAP + 12.0);
        let (first, lit) = window(self.paths.len(), self.highlighted, MENTIONS_SHOWN);
        menu = menu.highlighted(Some(lit));
        for path in self.paths.into_iter().skip(first).take(MENTIONS_SHOWN) {
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
    /// Emits the menu: a filter over the values, when there are many, and a row for each value it lets through, the current one ticked.
    #[track_caller]
    pub(crate) fn show(self) {
        let filter = remember(String::new);
        let many = self.choice.picks.len() > FILTER_FROM;
        let needle = filter.get().to_lowercase();
        let picks: Vec<ComposerPick> = self
            .choice
            .picks
            .iter()
            .filter(|pick| {
                !many
                    || pick.name.to_lowercase().contains(&needle)
                    || pick.value.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect();
        let pick_with = {
            let on_pick = self.on_pick.clone();
            let id = self.choice.id.clone();
            move |value: String| {
                if let Some(on_pick) = &on_pick {
                    on_pick(id.clone(), value);
                }
            }
        };
        let mut menu = Menu::new().size(Size::Xs);
        if !many {
            menu = menu.heading(self.choice.heading.clone());
        }
        for pick in &picks {
            let select = {
                let pick_with = pick_with.clone();
                let value = pick.value.clone();
                move || pick_with(value.clone())
            };
            menu = if pick.value == self.choice.current {
                menu.icon_item(phosphor::CHECK, pick.name.clone(), select)
            } else {
                menu.item(pick.name.clone(), select)
            };
            if !pick.description.is_empty() {
                menu = menu.hint(pick.description.clone());
            }
        }
        let dismiss = self.on_dismiss;
        if !many {
            menu.width(260.0)
                .above(true)
                .offset(0.0, GAP + BUTTON_HEIGHT)
                .on_dismiss(move || dismiss())
                .show();
            return;
        }
        let first = picks.first().map(|pick| pick.value.clone());
        let closer = dismiss.clone();
        let heading = self.choice.heading.clone();
        Popover::new()
            .width(300.0)
            .above(true)
            .offset(0.0, GAP + BUTTON_HEIGHT)
            .on_dismiss(move || dismiss())
            .show(move || {
                let typed = filter.clone();
                let submit_close = closer.clone();
                let submit = pick_with.clone();
                TextInput::new(filter.get())
                    .placeholder(format!("Filter {}", heading.to_lowercase()))
                    .autofocus()
                    .size(Size::Xs)
                    .modifier(Modifier::new().fill_max_width().padding(4.0))
                    .on_change(move |text| typed.set(text))
                    .on_submit(move |_| {
                        if let Some(value) = &first {
                            submit_close();
                            submit(value.clone());
                        }
                    })
                    .show();
                let scroll = remember_scroll_state();
                column_with(
                    Modifier::new()
                        .fill_max_width()
                        .height_in(0.0, FILTERED_HEIGHT)
                        .vertical_scroll(&scroll),
                    move || {
                        menu.variant(MenuVariant::Inline)
                            .width(292.0)
                            .on_dismiss(move || closer())
                            .show();
                    },
                );
            });
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
    /// The text area's focus, given back to it after each command.
    pub(crate) focus: rustle::Focus,
}

impl FieldMenu {
    /// Emits the menu.
    #[track_caller]
    pub(crate) fn show(self) {
        let callbacks = self.callbacks.clone();
        let dismiss = self.on_dismiss.clone();
        TextMenu::new(&self.state)
            .focus(&self.focus)
            .above(true)
            .on_paste(move || paste_attachments(&callbacks))
            .show_at(self.at, move || dismiss());
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
        assert_eq!(
            with_mention("look at @ma", "src/main.rs"),
            "look at @src/main.rs "
        );
        assert_eq!(with_mention("@", "a.rs"), "@a.rs ");
        let paths = [
            "src/domain.rs".to_owned(),
            "main.rs".to_owned(),
            "x.rs".to_owned(),
        ];
        assert_eq!(
            MentionMenu::matching(&paths, "main"),
            ["main.rs", "src/domain.rs"]
        );
    }

    /// A long list scrolls so the entry the keys point at stays in view.
    #[test]
    fn the_window_follows_the_highlight() {
        assert_eq!(window(20, 0, 8), (0, 0));
        assert_eq!(window(20, 7, 8), (0, 7));
        assert_eq!(window(20, 12, 8), (5, 7));
        assert_eq!(window(20, 19, 8), (12, 7));
        assert_eq!(window(3, 2, 8), (0, 2));
    }
}

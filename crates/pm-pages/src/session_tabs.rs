//! The session view's tab strip: the conversation, the diff, each open
//! file, the excerpts of a search, the browser and the release notes, the
//! preview tab leaning, a menu on each file tab, tabs dragged onto a pane,
//! the button that splits the view, and the keys that step through the
//! tabs, reopen a closed one and open the file showing to the side.

use rustle::prelude::*;
use rustle::{Key, KeyEvent};

use pm_components::{IconButton, Menu, Size, Tab};
use pm_shell::{BrowserAction, OpenFiles, Pane, SessionTab, TabAction, TabMenu, Workspace};

use crate::actions::Actions;

/// Emits the strip of tabs over the session view.
pub fn session_tabs(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    let files = workspace.open_files.clone();
    let browser = workspace.browser.clone();
    let update = workspace.update.is_some() && files.update_tab;
    let agent = if workspace.agent.is_empty() {
        "Chat".to_owned()
    } else {
        workspace.agent.clone()
    };
    let shown = shown_tab(&files);
    let actions = actions.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(36.0)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(8.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(2.0)),
        Alignment::Center,
        move || {
            plain_tab(
                &agent,
                phosphor::SPARKLE,
                SessionTab::Chat,
                shown,
                &actions,
                false,
            );
            plain_tab(
                "Diff",
                phosphor::GIT_DIFF,
                SessionTab::Diff,
                shown,
                &actions,
                false,
            );
            for index in 0..files.files.len() {
                key((index, files.files[index].path.clone()), || {
                    file_tab(&files, index, shown, &actions)
                });
            }
            if files.excerpts_tab {
                plain_tab(
                    "Excerpts",
                    phosphor::LIST_MAGNIFYING_GLASS,
                    SessionTab::Excerpts,
                    shown,
                    &actions,
                    true,
                );
            }
            if let Some(browser) = &browser {
                let show = actions.file_tab.clone();
                let close = actions.browser.clone();
                draggable(SessionTab::Browser, || {
                    Tab::new(browser.shown().label().chars().take(24).collect::<String>())
                        .icon(phosphor::GLOBE)
                        .size(Size::Xs)
                        .selected(shown == SessionTab::Browser)
                        .on_click(move || show(TabAction::Select(SessionTab::Browser)))
                        .on_close(move || close(BrowserAction::Close))
                        .show();
                });
            }
            if update {
                plain_tab(
                    "Update",
                    phosphor::ARROW_CIRCLE_UP,
                    SessionTab::Update,
                    shown,
                    &actions,
                    true,
                );
            }
            spacer(Modifier::new().weight(1.0));
            let open = actions.browser.clone();
            IconButton::new(phosphor::GLOBE)
                .size(Size::Xs)
                .active(shown == SessionTab::Browser)
                .on_click(move || open(BrowserAction::Open))
                .show();
            let split = actions.file_tab.clone();
            let is_split = files.split.is_some();
            IconButton::new(phosphor::SQUARE_SPLIT_HORIZONTAL)
                .size(Size::Xs)
                .active(is_split)
                .on_click(move || {
                    split(if is_split {
                        TabAction::Unsplit
                    } else {
                        TabAction::Split
                    })
                })
                .show();
        },
    );
}

/// The tab showing in the pane tabs chosen show in.
fn shown_tab(files: &OpenFiles) -> SessionTab {
    match (files.focus, files.right_tab()) {
        (Pane::Right, Some(tab)) => tab,
        _ => files.tab,
    }
}

/// Emits `content`, a tab, as something that can be dragged onto a pane to
/// show `tab` there.
fn draggable(tab: SessionTab, content: impl FnOnce()) {
    column_with(Modifier::new().drag_source(tab), content);
}

/// Emits the tab `tab` that is not a file's, reading `label` beside
/// `glyph`, selected when it is `shown`, with a close button when `closes`.
fn plain_tab(
    label: &str,
    glyph: rustle::Icon,
    tab: SessionTab,
    shown: SessionTab,
    actions: &Actions,
    closes: bool,
) {
    let select = actions.file_tab.clone();
    let close = actions.file_tab.clone();
    let label = label.to_owned();
    draggable(tab, move || {
        let mut chip = Tab::new(label)
            .icon(glyph)
            .size(Size::Xs)
            .selected(shown == tab)
            .on_click(move || select(TabAction::Select(tab)));
        if closes {
            chip = chip.on_close(move || close(TabAction::Dismiss(tab)));
        }
        chip.show();
    });
}

/// Emits the tab of the open file at `index`, with its menu.
fn file_tab(files: &OpenFiles, index: usize, shown: SessionTab, actions: &Actions) {
    let file = &files.files[index];
    let menu_open = files.menu == Some(TabMenu::Tab(index));
    let split = files.split.is_some();
    let act = actions.file_tab.clone();
    let name = file.name().to_owned();
    let preview = file.preview;
    let dirty = file.is_dirty();
    let markdown = file.is_markdown();
    let rendered = file.rendered;
    let path = file.path.clone();
    draggable(SessionTab::File(index), move || {
        let select = act.clone();
        let keep = act.clone();
        let close = act.clone();
        let opened = act.clone();
        Tab::new(name)
            .icon(phosphor::FILE_TEXT)
            .size(Size::Xs)
            .selected(shown == SessionTab::File(index))
            .preview(preview)
            .dirty(dirty)
            .on_click(move || select(TabAction::Select(SessionTab::File(index))))
            .on_double_click(move || keep(TabAction::Keep(index)))
            .on_close(move || close(TabAction::Close(index)))
            .on_menu(move || opened(TabAction::Menu(Some(TabMenu::Tab(index)))))
            .show();
        if menu_open {
            tab_menu(
                index,
                preview,
                markdown.then_some(rendered),
                split,
                path,
                &act,
            );
        }
    });
}

/// Emits the menu of the tab of the open file at `index`: closing it and
/// its neighbours, keeping a preview, rendering markdown when `markdown`
/// says how it shows, showing it in the other pane, copying its path and
/// reopening the tab closed last.
fn tab_menu(
    index: usize,
    preview: bool,
    markdown: Option<bool>,
    split: bool,
    path: String,
    act: &std::rc::Rc<dyn Fn(TabAction)>,
) {
    let dismiss = act.clone();
    let pick = |action: TabAction| {
        let act = act.clone();
        move || act(action)
    };
    let mut menu = Menu::new()
        .on_dismiss(move || dismiss(TabAction::Menu(None)))
        .item("Close", pick(TabAction::Close(index)))
        .hint("Ctrl+W")
        .item("Close others", pick(TabAction::CloseOthers(index)))
        .item("Close to the right", pick(TabAction::CloseRight(index)))
        .item("Close all", pick(TabAction::CloseAll))
        .separator();
    if preview {
        menu = menu.item("Keep open", pick(TabAction::Keep(index)));
    }
    if let Some(rendered) = markdown {
        let label = if rendered {
            "Show as text"
        } else {
            "Show rendered"
        };
        menu = menu.item(label, pick(TabAction::ToggleRendered(index)));
    }
    menu = menu
        .item(
            "Open to the side",
            pick(TabAction::MoveTo(SessionTab::File(index), Pane::Right)),
        )
        .hint("Ctrl+\\");
    if split {
        menu = menu.item(
            "Open in the left pane",
            pick(TabAction::MoveTo(SessionTab::File(index), Pane::Left)),
        );
    }
    let copied = act.clone();
    menu.item("Copy path", move || {
        if let Some(board) = rustle::clipboard() {
            board.set_text(&path);
        }
        copied(TabAction::Menu(None));
    })
    .item("Reopen closed tab", pick(TabAction::Reopen))
    .hint("Ctrl+Shift+T")
    .show();
}

/// What the tab keys pressed anywhere in the session view ask for: Ctrl+Tab
/// and Ctrl+Page Down step forward, with Shift or Page Up back, Ctrl+Shift+T
/// reopens the tab closed last, Ctrl+W closes the file showing, and Ctrl+\\
/// splits the view or joins its panes again.
pub fn tab_key(event: &KeyEvent, files: &OpenFiles) -> Option<TabAction> {
    if !event.pressed || !(event.modifiers.ctrl || event.modifiers.meta) {
        return None;
    }
    let shift = event.modifiers.shift;
    match &event.key {
        Key::Tab if shift => Some(TabAction::Previous),
        Key::Tab | Key::PageDown => Some(TabAction::Next),
        Key::PageUp => Some(TabAction::Previous),
        Key::Character(letter) if letter.eq_ignore_ascii_case("t") && shift => {
            Some(TabAction::Reopen)
        }
        Key::Character(letter) if letter.eq_ignore_ascii_case("w") && !shift => {
            match shown_tab(files) {
                SessionTab::File(index) => Some(TabAction::Close(index)),
                _ => None,
            }
        }
        Key::Character(letter) if letter == "\\" && !shift => match files.split {
            Some(_) => Some(TabAction::Unsplit),
            None => Some(TabAction::Split),
        },
        _ => None,
    }
}

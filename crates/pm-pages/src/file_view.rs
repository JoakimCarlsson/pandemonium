//! An open file in the session view: a header naming it with a menu of
//! what can be done with it, an offer to install the language it is written
//! in when the server has not got it, then its text in the code editor, a
//! comparison with another text, a markdown file rendered, or a picture.

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, CodeEditor, CodeEditorBar, CodeEditorPopup, CodeEditorVariant, DiffView,
    IconButton, Label, LabelVariant, Markdown, Menu, PictureView, ServerAsk, Size,
};
use pm_shell::{
    Block, Comparison, DebugModel, EditorBar, EditorPopup, Extension, FileAction, FileBody,
    OpenFile, TabAction, TabMenu, Workspace, grammar_by_ending, language_offer,
};

use crate::actions::Actions;
use crate::diff_page::diff_rows;

/// What the header of an open file needs besides the file: whether its menu
/// is open, the other open files to compare it with, by position and path,
/// and the language on offer for it.
#[derive(Clone)]
struct Around {
    /// Whether the menu of what can be done with the file is open.
    menu: bool,
    /// The other open files, by position and path.
    others: Vec<(usize, String)>,
    /// The language not installed that reads the file, when there is one.
    offer: Option<Extension>,
    /// How many columns a level of indentation is and whether it is a tab, as the settings of the file's language say, each when they say.
    indent: (Option<usize>, Option<bool>),
}

/// Emits the open file at `index` of `workspace`'s open files, if it is
/// open, with the editor's keys, breakpoints and inline values the
/// workspace holds for it.
pub fn open_file_view(workspace: &Workspace, index: usize, actions: &Actions) -> bool {
    let files = &workspace.open_files;
    let Some(file) = files.files.get(index) else {
        return false;
    };
    let language_known = file.document.0.borrow().language.is_some();
    let around = Around {
        menu: files.menu == Some(TabMenu::File(index)),
        others: files
            .files
            .iter()
            .enumerate()
            .filter(|(place, other)| *place != index && other.body == FileBody::Text)
            .map(|(place, other)| (place, other.path.clone()))
            .collect(),
        offer: workspace
            .extensions
            .as_deref()
            .filter(|_| !language_known && !files.offers_withheld && file.body == FileBody::Text)
            .and_then(|extensions| language_offer(extensions, &file.path, &files.declined))
            .cloned(),
        indent: file
            .document
            .0
            .borrow()
            .language
            .as_ref()
            .and_then(|language| files.indents.get(language))
            .copied()
            .unwrap_or_default(),
    };
    file_view(file, index, files.vim, &workspace.debug, &around, actions);
    true
}

/// Width of a rendered markdown file's column.
const READING_WIDTH: f32 = 720.0;

/// Emits the open file at `index` of `file`'s list, with `vim` keys when
/// asked, marking the breakpoints and inline values `debug` holds for it.
fn file_view(
    file: &OpenFile,
    index: usize,
    vim: bool,
    debug: &DebugModel,
    around: &Around,
    actions: &Actions,
) {
    let debug = debug.clone();
    let around = around.clone();
    let scheme = color_scheme();
    let file = file.clone();
    let actions = actions.clone();
    column_with(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .background(scheme.background),
        move || {
            file_header(&file, index, &around, &actions);
            if let Some(offer) = &around.offer {
                language_bar(offer, index, &actions);
            }
            key(
                (file.directory.clone(), file.path.clone()),
                || match &file.body {
                    FileBody::Text if file.compare.is_some() => {
                        if let Some(comparison) = &file.compare {
                            compared(&file, comparison, index, &actions);
                        }
                    }
                    FileBody::Loading => note("Reading the file…"),
                    FileBody::Failed(problem) => note(problem),
                    FileBody::Picture(bytes) => PictureView::new(bytes.clone())
                        .modifier(Modifier::new().fill_max_width().weight(1.0))
                        .show(),
                    FileBody::Text if file.is_markdown() && file.rendered => rendered(&file),
                    FileBody::Text => {
                        let (tab_size, hard_tabs) = around.indent;
                        let look = pm_components::code_look().with_indent(tab_size, hard_tabs);
                        pm_components::provide_code_look(look, || {
                            editor(&file, index, vim, &debug, &actions)
                        });
                    }
                },
            );
        },
    );
}

/// The lines, counted from 0, of `file` that `debug` holds breakpoints on.
fn breakpoints_in(file: &OpenFile, debug: &DebugModel) -> Vec<usize> {
    debug
        .breakpoints
        .iter()
        .filter(|breakpoint| breakpoint.path == file.path)
        .map(|breakpoint| breakpoint.line)
        .collect()
}

/// What the paused session of `debug` shows after lines of `file`, by line
/// counted from 0; the adapter may name the file whole or from its folder.
fn inline_values_in(file: &OpenFile, debug: &DebugModel) -> Vec<(usize, String)> {
    let whole = format!("{}/{}", file.directory.trim_end_matches('/'), file.path);
    debug
        .session
        .iter()
        .flat_map(|session| session.inline_values.iter())
        .filter(|value| {
            value.path == whole
                || value.path.ends_with(&format!("/{}", file.path))
                || value.path == file.path
        })
        .map(|value| (value.line, value.text.clone()))
        .collect()
}

/// The editor's bar for the bar `file` asks to open with.
fn opening_bar(file: &OpenFile) -> Option<CodeEditorBar> {
    file.bar.as_ref().map(|bar| match bar {
        EditorBar::Find(text) => CodeEditorBar::Find(text.clone()),
        EditorBar::Replace(text) => CodeEditorBar::Replace(text.clone()),
        EditorBar::GoTo(text) => CodeEditorBar::GoTo(text.clone()),
    })
}

/// Emits the code editor over the text of `file` at `index`, with the
/// breakpoints and inline values of `debug`; a press on a line number sets
/// or clears a breakpoint there.
fn editor(file: &OpenFile, index: usize, vim: bool, debug: &DebugModel, actions: &Actions) {
    let edited = actions.file_tab.clone();
    let code = actions.code.clone();
    let dismiss = actions.dismiss_popups.clone();
    let popup = file.popup.as_ref().map(|popup| match popup {
        EditorPopup::Actions => CodeEditorPopup::Actions,
        EditorPopup::Colour(index) => CodeEditorPopup::Colour(*index),
    });
    let enabled: Vec<usize> = file
        .actions
        .iter()
        .enumerate()
        .filter(|(_, action)| action.disabled.is_none())
        .map(|(place, _)| place)
        .collect();
    let offered = enabled
        .iter()
        .map(|place| file.actions[*place].title.clone())
        .collect();
    let save = actions.save_file.clone();
    let breakpoint = actions.toggle_breakpoint.clone();
    let path = file.path.clone();
    let language = file.document.0.borrow().language.clone();
    let variant = if file.read_only {
        CodeEditorVariant::Viewer
    } else {
        CodeEditorVariant::Editor
    };
    CodeEditor::new(file.document.0.clone())
        .variant(variant)
        .size(Size::Sm)
        .vim(vim)
        .language(language)
        .breakpoints(breakpoints_in(file, debug))
        .inline_values(inline_values_in(file, debug))
        .opening(opening_bar(file))
        .actions(offered)
        .popup(popup)
        .rename(file.rename.clone())
        .on_ask(move |ask| code(index, offered_action(ask, &enabled)))
        .on_dismiss(move || dismiss(index))
        .modifier(Modifier::new().fill_max_width().weight(1.0))
        .on_change(move || edited(TabAction::Keep(index)))
        .on_save(move || save(index))
        .on_gutter(move |line| breakpoint(path.clone(), line.saturating_sub(1)))
        .show();
}

/// Maps a [`ServerAsk::TakeAction`] that names a place among the `enabled`
/// actions the editor offered to the action's place in the server's full
/// list; any other `ask` is returned as it is.
fn offered_action(ask: ServerAsk, enabled: &[usize]) -> ServerAsk {
    match ask {
        ServerAsk::TakeAction(place) => {
            ServerAsk::TakeAction(enabled.get(place).copied().unwrap_or(place))
        }
        other => other,
    }
}

/// Emits the markdown text of `file` rendered, scrolled in a reading column.
fn rendered(file: &OpenFile) {
    let scroll = remember_scroll_state();
    let text = file.document.0.borrow().field.text().to_string();
    let parsed = match Block::markdown(text) {
        Block::Markdown { blocks, .. } => blocks,
        _ => Vec::new(),
    };
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .vertical_scroll(&scroll)
            .padding_xy(0.0, 20.0),
        Arrangement::Start,
        Alignment::Center,
        move || {
            column_with(Modifier::new().width(READING_WIDTH), move || {
                Markdown::new(parsed).show()
            });
        },
    );
}

/// Emits `text` in the middle of the file's area, while it cannot show the file.
fn note(text: &str) {
    let text = text.to_owned();
    column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::Center,
        Alignment::Center,
        move || {
            Label::new(text.clone())
                .variant(LabelVariant::Muted)
                .size(Size::Sm)
                .show()
        },
    );
}

/// Emits the difference between `file` at `index` and the other text of
/// `comparison`, under a bar naming the other side with a button closing it.
fn compared(file: &OpenFile, comparison: &Comparison, index: usize, actions: &Actions) {
    let scheme = color_scheme();
    let label = format!("{}  ↔  {}", comparison.label, file.path);
    let rows = diff_rows(&comparison.diff);
    let grammar = grammar_by_ending(&file.path);
    let close = actions.file_tab.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(28.0)
            .background(scheme.surface_container)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        move || {
            icon(phosphor::GIT_DIFF, 12.0, scheme.on_surface_variant);
            Label::new(label.clone())
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            let close = close.clone();
            Button::new("Close comparison")
                .variant(ButtonVariant::Ghost)
                .size(Size::Xs)
                .on_click(move || close(TabAction::EndCompare(index)))
                .show();
        },
    );
    DiffView::new(rows)
        .grammar(grammar)
        .size(Size::Sm)
        .modifier(Modifier::new().fill_max_width().weight(1.0))
        .show();
}

/// Emits the offer to install `offer`, the language the file at `index` is
/// written in, which the server has not got.
fn language_bar(offer: &Extension, index: usize, actions: &Actions) {
    let scheme = color_scheme();
    let install = actions.file_action.clone();
    let decline = actions.file_action.clone();
    let id = offer.id.clone();
    let declined = offer.id.clone();
    let text = format!(
        "Install {} to highlight this file and read it with a language server.",
        offer.name
    );
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(32.0)
            .background(scheme.surface_container)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        move || {
            icon(phosphor::PUZZLE_PIECE, 14.0, scheme.primary);
            Label::new(text.clone())
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            let install = install.clone();
            let id = id.clone();
            Button::new("Install")
                .variant(ButtonVariant::Primary)
                .size(Size::Xs)
                .on_click(move || install(index, FileAction::InstallLanguage(id.clone())))
                .show();
            let decline = decline.clone();
            let declined = declined.clone();
            Button::new("Not now")
                .variant(ButtonVariant::Ghost)
                .size(Size::Xs)
                .on_click(move || decline(index, FileAction::DeclineLanguage(declined.clone())))
                .show();
        },
    );
}

/// Emits the menu of what can be done with the file at `index`, comparing
/// it with `others` among them.
fn file_menu(index: usize, others: &[(usize, String)], actions: &Actions) {
    let close = actions.file_tab.clone();
    let act = |action: FileAction| {
        let run = actions.file_action.clone();
        move || run(index, action.clone())
    };
    let clipboard = actions.file_action.clone();
    let mut menu = Menu::new()
        .width(260.0)
        .offset(-230.0, 26.0)
        .on_dismiss(move || close(TabAction::Menu(None)))
        .item("Show in Files", act(FileAction::Reveal))
        .item("Saved versions…", act(FileAction::History))
        .separator()
        .heading("Compare with")
        .item("Clipboard", move || {
            let text = rustle::clipboard()
                .and_then(|board| board.text())
                .unwrap_or_default();
            clipboard(index, FileAction::CompareClipboard(text));
        })
        .item("Saved file", act(FileAction::CompareSaved))
        .item("Last commit", act(FileAction::CompareCommitted));
    for (other, path) in others.iter().take(6) {
        menu = menu.item(path.clone(), act(FileAction::CompareWith(*other)));
    }
    menu.item("Another file…", act(FileAction::ChooseCompare))
        .separator()
        .item("Organize imports", act(FileAction::OrganizeImports))
        .hint("Shift+Alt+O")
        .item("Fix all", act(FileAction::FixAll))
        .hint("Shift+Alt+.")
        .item("Format", act(FileAction::Format))
        .hint("Shift+Alt+F")
        .show();
}

/// Emits the header of `file` at `index`: its path, a toggle to render a
/// markdown file, Save while it has changes, and the menu of what can be
/// done with it.
fn file_header(file: &OpenFile, index: usize, around: &Around, actions: &Actions) {
    let menu_open = around.menu;
    let others = around.others.clone();
    let actions_inner = actions.clone();
    let scheme = color_scheme();
    let path = file.path.clone();
    let markdown = file.is_markdown() && file.body == FileBody::Text;
    let rendered = file.rendered;
    let dirty = file.is_dirty();
    let preview = file.preview;
    let read_only = file.read_only;
    let act = actions.file_tab.clone();
    let save = actions.save_file.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(30.0)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        move || {
            icon(phosphor::FILE_TEXT, 12.0, scheme.on_surface_variant);
            Label::new(path.clone())
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            if preview {
                Label::new("Preview")
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
            }
            if read_only {
                let unlock = act.clone();
                icon(phosphor::LOCK_SIMPLE, 12.0, scheme.on_surface_variant);
                Label::new("Read only")
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
                IconButton::new(phosphor::PENCIL_SIMPLE)
                    .size(Size::Xs)
                    .on_click(move || unlock(TabAction::ToggleReadOnly(index)))
                    .show();
            }
            if markdown {
                let toggle = act.clone();
                IconButton::new(if rendered {
                    phosphor::CODE
                } else {
                    phosphor::EYE
                })
                .size(Size::Xs)
                .tooltip(if rendered {
                    "Show source"
                } else {
                    "Show preview"
                })
                .active(rendered)
                .on_click(move || toggle(TabAction::ToggleRendered(index)))
                .show();
            }
            if dirty {
                let save = save.clone();
                Button::new("Save")
                    .variant(ButtonVariant::Secondary)
                    .size(Size::Xs)
                    .on_click(move || save(index))
                    .show();
            }
            let open = act.clone();
            let others = others.clone();
            let actions = actions_inner.clone();
            column_with(Modifier::new(), move || {
                IconButton::new(phosphor::DOTS_THREE)
                    .size(Size::Xs)
                    .tooltip("More actions")
                    .active(menu_open)
                    .on_click(move || {
                        open(TabAction::Menu(if menu_open {
                            None
                        } else {
                            Some(TabMenu::File(index))
                        }))
                    })
                    .show();
                if menu_open {
                    file_menu(index, &others, &actions);
                }
            });
        },
    );
}

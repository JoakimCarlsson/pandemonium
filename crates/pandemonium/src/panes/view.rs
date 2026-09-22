//! Drawing the pane tree: a split for every division, a pane for every leaf.
//!
//! The view walks the tree the window keeps and asks for each pane's
//! contents as it reaches it, so what is drawn is the tree itself rather
//! than a copy of it made beforehand. A pane is the same bar of tabs the
//! terminal panel wears, with the file in front beneath it and a ring around
//! it while it has the keyboard. Its tabs are carried by the pointer, and
//! both they and the pane leave their bounds behind as they paint, because
//! where a carried tab is let go of is the window's to answer.

use pm_ui::{
    Bounds, Div, Element, IconName, Measured, MenuItem, Styled, Tab, Theme, h_flex, icon_button,
    measured, menu_entry, menu_separator, split, tab, tab_bar, text, v_flex,
};

use crate::editor::{FileEntry, FileId, OpenFile, buffer_view, search_bar};
use crate::message::Message;

use super::tree::{Node, Pane, PaneId, PaneTree, SplitDirection};

/// What one pane is showing, read out of the files the window has open.
pub struct Contents {
    /// Every file open in the pane, for the bar of tabs above it.
    pub tabs: Vec<FileEntry>,
    /// The file in front, which is the one the pane draws.
    pub file: Option<OpenFile>,
    /// Where the pane leaves its bounds, for a drop to be resolved against.
    pub bounds: Bounds,
    /// Where its bar of tabs leaves its bounds, for the same reason.
    pub bar: Bounds,
    /// Where each of its tabs leaves its own, in the order they are drawn.
    pub tab_bounds: Vec<Bounds>,
}

/// Builds the whole tree of panes, `focused` when the window's own focus is.
pub fn pane_tree(
    theme: &Theme,
    tree: &PaneTree,
    focused: bool,
    contents: &dyn Fn(&Pane) -> Contents,
) -> Box<dyn Element<Message>> {
    node(theme, tree, tree.root(), focused, contents)
}

/// Builds one node: a split of further nodes, or the pane at a leaf.
fn node(
    theme: &Theme,
    tree: &PaneTree,
    node: &Node,
    focused: bool,
    contents: &dyn Fn(&Pane) -> Contents,
) -> Box<dyn Element<Message>> {
    match node {
        Node::Pane(pane) => Box::new(pane_view(
            theme,
            pane,
            contents(pane),
            focused && tree.focus() == pane.id(),
            tree.is_split(),
        )),
        Node::Split(node) => {
            let id = node.id();
            let mut element = split(node.axis()).on_resize(move |index, event, scale| {
                Message::ResizeSplit(id, index, event, scale)
            });
            for (child, share) in node.children().iter().zip(node.shares()) {
                element = element.child(*share, self::node(theme, tree, child, focused, contents));
            }
            Box::new(element)
        }
    }
}

/// Builds one pane: its bar of tabs, and the file in front beneath it.
fn pane_view(
    theme: &Theme,
    pane: &Pane,
    contents: Contents,
    focused: bool,
    divided: bool,
) -> Measured<Message> {
    let id = pane.id();
    let active = pane.active();
    let tabs = contents
        .tabs
        .into_iter()
        .zip(contents.tab_bounds)
        .map(|(file, bounds)| pane_tab(id, &file, active == Some(file.id), bounds))
        .collect::<Vec<_>>();
    let empty = contents.file.is_none();
    let searching = contents
        .file
        .as_ref()
        .map(|file| file.borrow())
        .filter(|document| document.search().is_open())
        .map(|document| search_bar(theme, id, document.search()));

    let body = v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .when(divided && focused, |pane| {
            pane.border_1(theme.colors.border_focused)
        })
        .when(!tabs.is_empty(), |view| {
            view.child(measured(
                contents.bar,
                tab_bar(theme, tabs, pane_actions(theme, id, divided)),
            ))
        })
        .when_some(searching, Div::child)
        .when_some(contents.file, |view, file| {
            view.child(
                buffer_view(file, focused)
                    .on_select(move |anchor, head| Message::SelectText(id, anchor, head))
                    .on_gutter(move |anchor, head| Message::SelectLines(id, anchor, head))
                    .on_fold(move |at| Message::ToggleFold(id, at))
                    .on_scroll(move |axis, event, step| {
                        Message::ScrollEditor(id, axis, event, step)
                    })
                    .on_menu(Message::ShowEditorMenu(id)),
            )
        })
        .when(empty, |view| view.child(placeholder(theme, id)));

    measured(contents.bounds, body)
}

/// Builds one tab of a pane: what it holds, and the drag that carries it.
fn pane_tab(pane: PaneId, file: &FileEntry, active: bool, bounds: Bounds) -> Tab<Message> {
    let id = file.id;

    tab(
        IconName::File,
        file.name.clone(),
        Message::SelectFile(pane, id),
        Message::CloseFile(pane, id),
        Message::ShowFileMenu(pane, id),
    )
    .active(active)
    .dirty(file.dirty)
    .preview(file.preview)
    .on_drag(bounds, move |event| Message::DragTab(pane, id, event))
}

/// Builds the pane's own controls, at the end of its bar of tabs.
fn pane_actions(theme: &Theme, id: PaneId, divided: bool) -> Div<Message> {
    h_flex()
        .h_full()
        .px(1.5)
        .gap(1)
        .items_center()
        .child(icon_button(
            theme,
            IconName::Split,
            Message::ShowPaneMenu(id),
        ))
        .when(divided, |actions| {
            actions.child(icon_button(theme, IconName::Close, Message::ClosePane(id)))
        })
}

/// Builds what an empty pane says, which is also what claims it for a click.
fn placeholder(theme: &Theme, id: PaneId) -> Div<Message> {
    v_flex()
        .w_full()
        .flex_1()
        .items_center()
        .justify_center()
        .on_click(Message::FocusPane(id))
        .child(
            text("Open a file from the tree")
                .text_sm()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// The ways one pane can be divided, and what else can be done to it.
///
/// The four directions are the whole menu, as they are in Zed: splitting is
/// the one thing a pane does to itself, and closing is the one thing that
/// undoes it.
pub fn pane_menu(id: PaneId, divided: bool) -> Vec<MenuItem<Message>> {
    let mut items = SplitDirection::ALL
        .into_iter()
        .map(|direction| menu_entry(direction.label(), Some(Message::SplitPane(id, direction))))
        .collect::<Vec<_>>();
    items.push(menu_separator());
    items.push(menu_entry(
        "Close Pane",
        divided.then_some(Message::ClosePane(id)),
    ));
    items
}

/// What can be done about a file being closed with changes that are not saved.
///
/// Closing a file is not something to be quietly wrong about, so the only
/// way past this menu is to say which of the two things should happen. The
/// sheet under it dismisses it, which leaves the file open.
pub fn unsaved_menu(pane: PaneId, file: FileId, name: &str) -> Vec<MenuItem<Message>> {
    vec![
        menu_entry(
            format!("{name} has changes that are not saved"),
            None::<Message>,
        ),
        menu_separator(),
        menu_entry("Save and Close", Some(Message::SaveAndClose(pane, file))),
        menu_entry(
            "Close Without Saving",
            Some(Message::DiscardAndClose(pane, file)),
        ),
        menu_separator(),
        menu_entry("Cancel", Some(Message::DismissMenu)),
    ]
}

/// The things that can be done to one tab of `pane`, given what else it holds.
///
/// An entry that does not apply — closing what is left of the first tab,
/// closing others when there are none — is greyed rather than left out, so
/// the menu keeps its shape wherever it is opened.
pub fn file_menu(pane: &Pane, tabs: &[FileEntry], target: FileId) -> Vec<MenuItem<Message>> {
    let id = pane.id();
    let index = tabs.iter().position(|file| file.id == target);
    let others = tabs.len() > 1;
    let left = index.is_some_and(|index| index > 0);
    let right = index.is_some_and(|index| index + 1 < tabs.len());
    let saved = tabs.iter().any(|file| !file.dirty);
    let preview = tabs.iter().any(|file| file.id == target && file.preview);

    vec![
        menu_entry(
            "Keep Open",
            preview.then_some(Message::KeepFileOpen(target)),
        ),
        menu_separator(),
        menu_entry(
            "Split Right",
            Some(Message::SplitFile(id, target, SplitDirection::Right)),
        ),
        menu_entry(
            "Split Down",
            Some(Message::SplitFile(id, target, SplitDirection::Down)),
        ),
        menu_separator(),
        menu_entry("Close", Some(Message::CloseFile(id, target))),
        menu_entry(
            "Close Others",
            others.then_some(Message::CloseOtherFiles(id, target)),
        ),
        menu_separator(),
        menu_entry(
            "Close Left",
            left.then_some(Message::CloseFilesLeft(id, target)),
        ),
        menu_entry(
            "Close Right",
            right.then_some(Message::CloseFilesRight(id, target)),
        ),
        menu_separator(),
        menu_entry("Close Saved", saved.then_some(Message::CloseSavedFiles(id))),
        menu_entry("Close All", Some(Message::CloseAllFiles(id))),
        menu_separator(),
        menu_entry("Copy Path", Some(Message::CopyFilePath(target))),
        menu_entry(
            "Copy Relative Path",
            Some(Message::CopyFileRelativePath(target)),
        ),
        menu_separator(),
        menu_entry("Reveal in File Manager", Some(Message::RevealFile(target))),
        menu_entry(
            "Open in Terminal",
            Some(Message::OpenFileInTerminal(target)),
        ),
    ]
}

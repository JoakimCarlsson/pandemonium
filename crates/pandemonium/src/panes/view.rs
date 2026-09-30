//! Drawing the pane tree: a split for every division, a pane for every leaf.
//!
//! The view walks the tree the window keeps and asks for each pane's
//! contents as it reaches it, so what is drawn is the tree itself rather
//! than a copy of it made beforehand. A pane is a bar of tabs with whatever
//! is in front beneath it, its tab in front lit while it has the keyboard. Its tabs are carried by the pointer,
//! and both they and the pane leave their bounds behind as they paint,
//! because where a carried tab is let go of is the window's to answer.

use pm_ui::{
    Bounds, Div, Element, IconName, Measured, MenuItem, Styled, Tab, Theme, h_flex, icon_button,
    kbd, measured, menu_entry, menu_separator, split, tab, tab_bar, text, v_flex,
};

use pm_core::Scope;

use crate::editor::{Breakpoint, Crumbs, Display, OpenFile, buffer_view, crumb_bar, search_bar};
use crate::excerpts::{OpenExcerpts, excerpts_view};
use crate::message::Message;
use crate::panes::item::Item;
use crate::review::Remarking;

use super::tree::{Node, Pane, PaneId, PaneTree, SplitDirection};

/// Width the keys of an empty pane's commands are right-aligned in, so the
/// names beside them start in one column.
const SHORTCUT_KEYS_WIDTH: f32 = 120.0;

/// One tab of a pane as its bar presents it.
///
/// The bar draws a name, an icon and three marks whatever the tab holds; what
/// it holds is only how the window works out what to put in them.
pub struct TabEntry {
    /// What the tab holds.
    pub item: Item,
    /// What the tab calls it.
    pub name: String,
    /// What the tab shows before the name.
    pub icon: IconName,
    /// Whether what it holds has changes that are not on disk.
    pub dirty: bool,
    /// Whether it is only being previewed, and will give its tab up.
    pub preview: bool,
    /// Whether its pane keeps it through a change of project.
    pub pinned: bool,
}

/// What a pane draws beneath its bar of tabs.
///
/// A file is drawn by the editor from the document itself, because the pane
/// and the window are looking at one buffer. Everything else arrives already
/// built: the window knows what a review or a session is, and the pane tree
/// is not the place to learn.
pub enum Content {
    /// Nothing at all, which is a pane with no tabs left in it.
    Empty,
    /// A file, drawn from the document the window has open.
    File(OpenFile),
    /// A worktree's changes as excerpts, drawn from the documents behind them.
    Excerpts(OpenExcerpts, Remarking),
    /// A screen the window built, drawn as it arrived.
    Built(Box<dyn Element<Message>>),
}

/// What one pane is showing, read out of what the window has open.
pub struct Contents {
    /// Every tab the pane shows, for the bar above it.
    pub tabs: Vec<TabEntry>,
    /// The tab in front, which is the one the bar lights.
    pub active: Option<Item>,
    /// What the pane draws beneath the bar.
    pub content: Content,
    /// Whether the open file has an unresolved Git conflict.
    pub conflicted: bool,
    /// Where the pane leaves its bounds, for a drop to be resolved against.
    pub bounds: Bounds,
    /// Where its bar of tabs leaves its bounds, for the same reason.
    pub bar: Bounds,
    /// Where each of its tabs leaves its own, in the order they are drawn.
    pub tab_bounds: Vec<Bounds>,
    /// The name the pointer is over in it, while the link key is held.
    pub link: Option<std::ops::Range<pm_text::Position>>,
    /// The name the editor is saying something about, while it says it.
    pub hovered: Option<std::ops::Range<pm_text::Position>>,
    /// The matches of modal editing's search, to light on screen.
    pub found: Vec<std::ops::Range<pm_text::Position>>,
    /// The breakpoints of the file in front, to mark in its gutter.
    pub breakpoints: Vec<Breakpoint>,
    /// The line a paused program stands on in the file in front, if it does.
    pub stopped: Option<usize>,
    /// Whether the caret is solid this instant, for its blink.
    pub caret: bool,
    /// Whether inline predictions are visible in this pane.
    pub prediction_visible: bool,
    /// What a pane of text draws around its text.
    pub display: Display,
    /// Where the text in front is, for the bar above it, when it is drawn.
    pub crumbs: Option<Crumbs>,
    /// The commands an empty pane offers, with the keys they answer to.
    pub shortcuts: Vec<Shortcut>,
}

/// One command an empty pane offers, and the keys it answers to.
#[derive(Clone, Debug)]
pub struct Shortcut {
    /// What the command is called.
    pub title: &'static str,
    /// The keys it answers to, as the keymap writes them.
    pub keys: String,
}

/// Builds the tree of panes `scope` draws, `focused` when the window's own
/// focus is.
pub fn pane_tree(
    theme: &Theme,
    tree: &PaneTree,
    scope: Option<Scope>,
    focused: bool,
    contents: &dyn Fn(&Pane) -> Contents,
) -> Box<dyn Element<Message>> {
    let divided = tree.drawn(scope).len() > 1;
    let drawing = Drawing {
        theme,
        tree,
        scope,
        focused,
        divided,
        contents,
    };
    drawing.node(tree.root())
}

/// What every node of one drawing of the tree is built with.
struct Drawing<'a> {
    /// The theme the panes are drawn in.
    theme: &'a Theme,
    /// The tree being drawn.
    tree: &'a PaneTree,
    /// The worktree the window is showing.
    scope: Option<Scope>,
    /// Whether the window's own focus is on the panes.
    focused: bool,
    /// Whether more than one pane is drawn.
    divided: bool,
    /// What each pane is showing.
    contents: &'a dyn Fn(&Pane) -> Contents,
}

impl Drawing<'_> {
    /// Builds one node: a split of the children `scope` draws, or the pane
    /// at a leaf. A split drawing one child is no division, so that child is
    /// drawn in its place.
    fn node(&self, node: &Node) -> Box<dyn Element<Message>> {
        match node {
            Node::Pane(pane) => Box::new(pane_view(
                self.theme,
                pane,
                (self.contents)(pane),
                self.focused && self.tree.focus() == pane.id(),
                self.divided,
            )),
            Node::Split(node) => {
                let drawn = node.drawn(self.scope);
                if let [only] = drawn[..] {
                    return self.node(&node.children()[only]);
                }
                let id = node.id();
                let mut element = split(node.axis()).on_resize(move |index, event, scale| {
                    Message::ResizeSplit(id, index, event, scale)
                });
                for index in drawn {
                    element =
                        element.child(node.shares()[index], self.node(&node.children()[index]));
                }
                Box::new(element)
            }
        }
    }
}

/// Builds one pane: its bar of tabs, and what is in front beneath it.
fn pane_view(
    theme: &Theme,
    pane: &Pane,
    contents: Contents,
    focused: bool,
    divided: bool,
) -> Measured<Message> {
    let id = pane.id();
    let active = contents.active;
    let previewable = contents.tabs.iter().any(|tab| {
        Some(tab.item) == active
            && matches!(tab.item, Item::File(_) | Item::Image(_))
            && (tab.name.to_ascii_lowercase().ends_with(".svg")
                || [".md", ".markdown", ".mdown", ".mkd"]
                    .iter()
                    .any(|ending| tab.name.to_ascii_lowercase().ends_with(ending)))
    });
    let tabs = contents
        .tabs
        .into_iter()
        .zip(contents.tab_bounds)
        .map(|(held, bounds)| pane_tab(id, &held, active == Some(held.item), bounds))
        .collect::<Vec<_>>();
    let empty = matches!(contents.content, Content::Empty);
    let shortcuts = contents.shortcuts;
    let link = contents.link.clone();
    let hovered = contents.hovered.clone();
    let found = contents.found.clone();
    let breakpoints = contents.breakpoints.clone();
    let stopped = contents.stopped;
    let caret = contents.caret;
    let prediction_visible = contents.prediction_visible;
    let display = contents.display;
    let showing = match &contents.content {
        Content::File(file) => Some(file.clone()),
        _ => None,
    };
    let conflict_file = active.and_then(Item::file).filter(|_| contents.conflicted);
    let excerpted = match &contents.content {
        Content::Excerpts(excerpts, remarking) => Some((excerpts.clone(), *remarking)),
        _ => None,
    };
    let searching = showing
        .as_ref()
        .map(|file| file.borrow())
        .filter(|document| document.search().is_open())
        .map(|document| search_bar(theme, id, document.search(), caret));

    let body = v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .when(divided && focused && tabs.is_empty(), |pane| {
            pane.border_1(theme.colors.border_focused)
        })
        .when(!tabs.is_empty(), |view| {
            view.child(measured(
                contents.bar,
                tab_bar(
                    theme,
                    tabs,
                    pane_actions(theme, id, divided, previewable),
                    focused,
                ),
            ))
        })
        .when_some(
            contents.crumbs.filter(|_| showing.is_some()),
            |view, crumbs| view.child(crumb_bar(theme, id, &crumbs)),
        )
        .when_some(searching, Div::child)
        .when_some(showing, |view, file| {
            let editor = buffer_view(file, focused)
                .prediction_visible(prediction_visible)
                .link(link)
                .hovered(hovered)
                .found(found)
                .breakpoints(breakpoints)
                .stopped(stopped)
                .caret(caret)
                .display(display)
                .on_select(move |phase, anchor, head| Message::SelectText(id, phase, anchor, head))
                .on_gutter(move |anchor, head| Message::SelectLines(id, anchor, head))
                .on_fold(move |at| Message::ToggleFold(id, at))
                .on_breakpoint(move |at| Message::ToggleBreakpoint(id, at))
                .on_breakpoint_menu(move |at| Message::ShowBreakpointMenu(id, at))
                .on_scroll(move |axis, event, step| Message::ScrollEditor(id, axis, event, step))
                .on_minimap(move |line| Message::ScrollEditorTo(id, line))
                .on_menu(Message::ShowEditorMenu(id));
            let editor = match conflict_file {
                Some(file) => editor
                    .on_conflict(move |line, action| Message::ConflictAction(file, line, action)),
                None => editor,
            };
            view.child(editor)
        })
        .when_some(excerpted, |view, (excerpts, remarking)| {
            view.child(
                excerpts_view(excerpts, focused)
                    .caret(caret)
                    .on_select(move |phase, file, anchor, head| {
                        Message::SelectExcerpt(id, phase, file, anchor, head)
                    })
                    .on_open(move |index| Message::OpenExcerptFile(id, index))
                    .on_comment(Message::CommentExcerpt)
                    .remarks(
                        |theme, comment, unit, inset, moving| {
                            crate::review::comment_block(theme, comment, unit, inset, moving)
                        },
                        move |theme, composing, unit, inset| {
                            crate::review::composer_block(
                                theme,
                                composing,
                                unit,
                                inset,
                                remarking.focused,
                                remarking.solid,
                            )
                        },
                    ),
            )
        })
        .when(empty, |view| view.child(placeholder(theme, id, &shortcuts)))
        .when_some(built(contents.content), Div::child);

    measured(contents.bounds, body)
}

/// The screen a pane was handed, when what it holds is one.
fn built(content: Content) -> Option<Box<dyn Element<Message>>> {
    match content {
        Content::Built(screen) => Some(screen),
        Content::Empty | Content::File(_) | Content::Excerpts(..) => None,
    }
}

/// Builds one tab of a pane: what it holds, and the drag that carries it.
fn pane_tab(pane: PaneId, held: &TabEntry, active: bool, bounds: Bounds) -> Tab<Message> {
    let item = held.item;

    tab(
        held.icon,
        held.name.clone(),
        Message::SelectItem(pane, item),
        Message::CloseItem(pane, item),
        Message::ShowTabMenu(pane, item),
    )
    .active(active)
    .dirty(held.dirty)
    .preview(held.preview)
    .pinned(held.pinned, Message::TogglePin(pane, item))
    .on_drag(bounds, move |event| Message::DragTab(pane, item, event))
}

/// Builds the pane's own controls, at the end of its bar of tabs.
fn pane_actions(theme: &Theme, id: PaneId, divided: bool, previewable: bool) -> Div<Message> {
    h_flex()
        .h_full()
        .px(1.5)
        .gap(1)
        .items_center()
        .when(previewable, |actions| {
            actions.child(icon_button(theme, IconName::Eye, Message::PreviewFile(id)))
        })
        .child(icon_button(
            theme,
            IconName::Split,
            Message::ShowPaneMenu(id),
        ))
        .when(divided, |actions| {
            actions.child(icon_button(theme, IconName::Close, Message::ClosePane(id)))
        })
}

/// Builds what an empty pane says, which is also what claims it for a click:
/// what the pane is for, and the keys that fill it.
fn placeholder(theme: &Theme, id: PaneId, shortcuts: &[Shortcut]) -> Div<Message> {
    v_flex()
        .w_full()
        .flex_1()
        .gap(4)
        .items_center()
        .justify_center()
        .on_click(Message::FocusPane(id))
        .child(
            v_flex()
                .gap(1.5)
                .items_center()
                .child(
                    text("Nothing open here")
                        .text_base()
                        .font_semibold()
                        .color(theme.colors.text),
                )
                .child(
                    text("Open a file from the tree, or start from the keyboard.")
                        .text_sm()
                        .color(theme.colors.text_muted),
                ),
        )
        .child(
            v_flex().gap(1.5).children(
                shortcuts
                    .iter()
                    .map(|shortcut| shortcut_row(theme, shortcut)),
            ),
        )
}

/// Builds one line of an empty pane's keys: the keys, then what they do.
fn shortcut_row(theme: &Theme, shortcut: &Shortcut) -> Div<Message> {
    h_flex()
        .gap(3)
        .items_center()
        .child(
            h_flex()
                .w_px(SHORTCUT_KEYS_WIDTH)
                .justify_end()
                .child(kbd(theme, shortcut.keys.clone())),
        )
        .child(
            text(shortcut.title)
                .text_sm()
                .color(theme.colors.text_muted),
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
pub fn unsaved_menu(
    pane: PaneId,
    file: crate::editor::FileId,
    name: &str,
) -> Vec<MenuItem<Message>> {
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
/// copying the path of something that is not a file — is greyed rather than
/// left out, so the menu keeps its shape wherever it is opened.
pub fn tab_menu(pane: &Pane, tabs: &[TabEntry], target: Item) -> Vec<MenuItem<Message>> {
    let id = pane.id();
    let index = tabs.iter().position(|held| held.item == target);
    let others = tabs.len() > 1;
    let left = index.is_some_and(|index| index > 0);
    let right = index.is_some_and(|index| index + 1 < tabs.len());
    let saved = tabs.iter().any(|held| !held.dirty);
    let preview = tabs.iter().any(|held| held.item == target && held.preview);
    let pinned = tabs.iter().any(|held| held.item == target && held.pinned);
    let file = target.file();

    vec![
        menu_entry(
            "Keep Open",
            file.filter(|_| preview).map(Message::KeepFileOpen),
        ),
        menu_entry(
            if pinned { "Unpin Tab" } else { "Pin Tab" },
            Some(Message::TogglePin(id, target)),
        ),
        menu_separator(),
        menu_entry(
            "Split Right",
            Some(Message::SplitItem(id, target, SplitDirection::Right)),
        ),
        menu_entry(
            "Split Down",
            Some(Message::SplitItem(id, target, SplitDirection::Down)),
        ),
        menu_separator(),
        menu_entry("Close", Some(Message::CloseItem(id, target))),
        menu_entry(
            "Close Others",
            others.then_some(Message::CloseOtherTabs(id, target)),
        ),
        menu_separator(),
        menu_entry(
            "Close Left",
            left.then_some(Message::CloseTabsLeft(id, target)),
        ),
        menu_entry(
            "Close Right",
            right.then_some(Message::CloseTabsRight(id, target)),
        ),
        menu_separator(),
        menu_entry("Close Saved", saved.then_some(Message::CloseSavedTabs(id))),
        menu_entry("Close All", Some(Message::CloseAllTabs(id))),
        menu_separator(),
        menu_entry("Copy Path", file.map(Message::CopyFilePath)),
        menu_entry(
            "Copy Relative Path",
            file.map(Message::CopyFileRelativePath),
        ),
        menu_separator(),
        menu_entry("Reveal in File Manager", file.map(Message::RevealFile)),
        menu_entry("Open in Terminal", file.map(Message::OpenFileInTerminal)),
        menu_entry(
            "Show Outline",
            file.map(|file| Message::OpenOutline(id, file)),
        ),
    ]
}

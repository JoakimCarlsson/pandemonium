//! The file tree as it is drawn: a header with its tools, then the rows.
//!
//! A row shows what the reader is doing to it as well as what it is: lit
//! when selected, outlined when the keyboard is on it, faded while it waits
//! to be moved by a paste, washed while rows carried over it would land in
//! it, and turned into a field while its name is being typed.

use std::path::Path;

use pm_core::{FileStatus, FileTree, Row};
use pm_ui::{
    Bounds, Div, IconName, IconSize, Scrolled, Styled, Theme, field, h_flex, icon, icon_button,
    measured, scroll_area, text, v_flex,
};

use crate::message::Message;
use crate::review::{Review, status_color};
use crate::tree::{Clipboard, Edit, EditKind, Selection};

/// How far the first level of the file tree sits from the edge.
const FILE_INSET: f32 = 6.0;

/// How far each further level of the file tree is indented.
const FILE_INDENT: f32 = 14.0;

/// How many rows of empty space are left under the last one, to drop onto
/// and to open the tree's own menu from.
const TRAILING_ROWS: f32 = 3.0;

/// Everything the tree is drawn from.
pub struct Listing<'a> {
    /// What the disk holds, as far as it has been read.
    pub tree: &'a FileTree,
    /// What git makes of the worktree, once it has been asked.
    pub review: Option<&'a Review>,
    /// Which rows are selected, and which one the keyboard is on.
    pub selection: Option<&'a Selection>,
    /// The name being typed into the tree, if one is.
    pub edit: Option<&'a Edit>,
    /// What was cut or copied out of the tree.
    pub clipboard: Option<&'a Clipboard>,
    /// The directory rows carried over the tree would land in.
    pub dropping: Option<&'a Path>,
    /// Whether keystrokes go to the tree.
    pub focused: bool,
    /// Whether the input caret is in its visible blink phase.
    pub caret: bool,
    /// How far the rows are scrolled.
    pub scroll: Scrolled,
    /// Where the rows came out, for telling which one the pointer is over.
    pub rows: Bounds,
    /// Where the scrolled area came out, for the wheel.
    pub area: Bounds,
    /// Where the name being typed came out, for a press that lands on it.
    pub field: Bounds,
}

/// Builds the list of every file of the worktree, or says there is none.
pub fn files_sidebar(theme: &Theme, listing: Option<&Listing<'_>>, width: f32) -> Div<Message> {
    let sidebar = v_flex().w_px(width).flex_1().overflow_hidden();
    let Some(listing) = listing else {
        return sidebar.child(
            text("No project open")
                .text_sm()
                .font_light()
                .color(theme.colors.text_subtle)
                .px(3)
                .py(2),
        );
    };

    sidebar.child(header(theme, listing.tree)).child(measured(
        listing.area.clone(),
        scroll_area(listing.scroll.clone(), body(theme, listing))
            .w_full()
            .flex_1(),
    ))
}

/// Builds the line above the tree: the worktree it lists and its tools.
fn header(theme: &Theme, tree: &FileTree) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pl(1.5)
        .pr(1)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .child(
            text(crate::workspace::shortened(tree.root()))
                .text_sm()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .child(h_flex().flex_1())
        .child(icon_button(theme, IconName::FileAdd, Message::NewTreeFile).tooltip("New File…"))
        .child(
            icon_button(theme, IconName::FolderAdd, Message::NewTreeFolder).tooltip("New Folder…"),
        )
        .child(icon_button(theme, IconName::Refresh, Message::RefreshTree).tooltip("Refresh"))
        .child(
            icon_button(theme, IconName::Collapse, Message::CollapseTree)
                .tooltip("Collapse Folders"),
        )
}

/// Builds the rows, the name being typed among them, and the space below.
fn body(theme: &Theme, listing: &Listing<'_>) -> Div<Message> {
    let rows = listing.tree.rows();
    let dropping_root = listing.dropping == Some(listing.tree.root());
    let mut lines = Vec::with_capacity(rows.len() + 2);

    if let Some(edit) = listing
        .edit
        .filter(|edit| creates_in(edit, listing.tree.root()))
    {
        lines.extend(edit_lines(theme, listing, edit, 0));
    }
    for row in &rows {
        let path = row.entry.path();
        match listing.edit {
            Some(edit) if edit.kind() == EditKind::Rename && edit.at() == path => {
                lines.extend(edit_lines(theme, listing, edit, row.depth));
            }
            _ => lines.push(file_row(theme, listing, row)),
        }
        if let Some(edit) = listing.edit.filter(|edit| creates_in(edit, path)) {
            lines.extend(edit_lines(theme, listing, edit, row.depth + 1));
        }
    }

    v_flex()
        .w_full()
        .when(dropping_root, |body| body.bg(theme.colors.drop_target))
        .child(measured(
            listing.rows.clone(),
            v_flex().w_full().children(lines),
        ))
        .child(
            v_flex()
                .w_full()
                .flex_1()
                .h_px(theme.size.row * TRAILING_ROWS)
                .on_click(Message::PressTreeSpace)
                .on_secondary_click(Message::ShowTreeMenu),
        )
}

/// Whether `edit` makes a new entry directly inside `directory`.
fn creates_in(edit: &Edit, directory: &Path) -> bool {
    edit.kind() != EditKind::Rename && edit.at() == directory
}

/// Builds the field a name is typed into, and what is wrong with the name.
fn edit_lines(
    theme: &Theme,
    listing: &Listing<'_>,
    edit: &Edit,
    depth: usize,
) -> Vec<Div<Message>> {
    let directory = match edit.kind() {
        EditKind::NewFolder => true,
        EditKind::NewFile => false,
        EditKind::Rename => edit.at().is_dir(),
    };
    let glyph = match directory {
        true => IconName::Folder,
        false => IconName::File,
    };
    let problem = edit.problem().filter(|_| !edit.is_unchanged());
    let border = match problem {
        Some(_) => theme.colors.danger,
        None => theme.colors.border_focused,
    };

    let line = h_flex()
        .w_full()
        .h_px(theme.size.row)
        .gap(0.5)
        .items_center()
        .child(indent(depth))
        .child(v_flex().w_px(IconSize::Medium.pixels()))
        .child(
            icon(glyph)
                .size(IconSize::Medium)
                .color(theme.colors.text_subtle),
        )
        .child(v_flex().w(1))
        .child(measured(
            listing.field.clone(),
            field(edit.field().value(), edit.field().caret(), listing.caret)
                .flex_1()
                .h_px(theme.size.row - 2.0)
                .px(1)
                .border_1(border)
                .bg(theme.colors.background)
                .on_press(Message::PlaceTreeEdit),
        ))
        .child(v_flex().w(1));

    let mut lines = vec![line];
    if let Some(problem) = problem {
        lines.push(
            h_flex()
                .w_full()
                .px(2)
                .py(1)
                .bg(theme.colors.surface)
                .border_1(theme.colors.danger)
                .child(
                    text(problem)
                        .text_xs()
                        .font_light()
                        .color(theme.colors.danger),
                ),
        );
    }
    lines
}

/// Builds the space a row at `depth` is indented by.
fn indent(depth: usize) -> Div<Message> {
    v_flex().w_px(FILE_INSET + depth as f32 * FILE_INDENT)
}

/// Builds one line of the file tree: chevron, icon and name.
fn file_row(theme: &Theme, listing: &Listing<'_>, row: &Row<'_>) -> Div<Message> {
    let entry = row.entry;
    let path = entry.path();
    let directory = entry.is_directory();
    let id = entry.id();
    let selected = listing
        .selection
        .is_some_and(|selection| selection.is_selected(path));
    let cursor = listing.focused
        && listing
            .selection
            .and_then(Selection::cursor)
            .is_some_and(|cursor| cursor == path);
    let cut = listing
        .clipboard
        .is_some_and(|clipboard| clipboard.is_cut(path));
    let dropping = listing
        .dropping
        .is_some_and(|target| target != listing.tree.root() && path.starts_with(target));
    let status = listing.review.and_then(|review| review.mark(path));
    let chevron = match (directory, row.expanded) {
        (false, _) => None,
        (true, true) => Some(IconName::ChevronDown),
        (true, false) => Some(IconName::ChevronRight),
    };
    let glyph = match (directory, row.expanded) {
        (false, _) => IconName::File,
        (true, true) => IconName::FolderOpen,
        (true, false) => IconName::Folder,
    };
    let background = match (dropping, selected, listing.focused) {
        (true, _, _) => Some(theme.colors.drop_target),
        (false, true, true) => Some(theme.colors.surface_selected),
        (false, true, false) => Some(theme.colors.surface_active),
        (false, false, _) => None,
    };

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .overflow_hidden()
        .gap(0.5)
        .items_center()
        .when_some(background, |line, color| line.bg(color))
        .when(cursor, |line| line.border_1(theme.colors.border_focused))
        .hover_bg(theme.colors.surface_hover)
        .on_drag(move |event| Message::PressEntry(id, event))
        .on_secondary_click(Message::ShowEntryMenu(id))
        .child(indent(row.depth))
        .child(
            h_flex()
                .w_px(IconSize::Medium.pixels())
                .items_center()
                .justify_center()
                .when_some(chevron, |slot, chevron| {
                    slot.child(
                        icon(chevron)
                            .size(IconSize::Medium)
                            .color(theme.colors.text_subtle),
                    )
                }),
        )
        .child(
            icon(glyph)
                .size(IconSize::Medium)
                .color(theme.colors.text_subtle),
        )
        .child(v_flex().w(1))
        .child(text(entry.name().to_owned()).color(name_color(theme, status, directory, cut)))
}

/// The colour a row's name is written in.
fn name_color(
    theme: &Theme,
    status: Option<FileStatus>,
    directory: bool,
    cut: bool,
) -> pm_gfx::Rgba {
    match (cut, status, directory) {
        (true, _, _) => theme.colors.text_subtle,
        (false, Some(status), _) => status_color(theme, status),
        (false, None, true) => theme.colors.text,
        (false, None, false) => theme.colors.text_muted,
    }
}

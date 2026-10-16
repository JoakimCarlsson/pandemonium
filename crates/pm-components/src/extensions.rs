//! The extensions page: what the server on this machine has installed, then what it can install, narrowed by a search, each a row saying what the extension gives the files of a project and whether it works, and the button that installs one from a folder.

use pm_icon::{IconName, Svg};
use pm_input::FocusId;
use pm_protocol::ExtensionInfo;
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, flex, flex_1, flex_col, flex_none,
    font_medium, font_mono, gap, h, items_center, justify_between, max_w_px, min_h, min_w_px,
    mx_auto, overflow_hidden, pb, pt, px, py, rounded, size, text_color, text_px, w_full,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::input::{self, Input};
use crate::parts::{glyph, line};
use crate::state::{Outgoing, WorkspaceState};

/// Opens the extensions page.
pub const EXTENSIONS: FocusId = FocusId(308);

/// The box the extensions are searched with.
pub const EXTENSIONS_SEARCH: FocusId = FocusId(9_000_000);

/// The scrolling list of extensions.
pub const EXTENSIONS_SCROLL: FocusId = FocusId(9_000_001);

/// Asks for a folder and installs the extension in it on the server on this machine.
pub const INSTALL_EXTENSION: FocusId = FocusId(9_000_002);

/// The first of the buttons installing or uninstalling an extension, in the order the page lists them; the rest follow it.
const ROW_BASE: u64 = 9_001_000;

/// One past the last name the page answers to.
const EXTENSIONS_END: u64 = 9_100_000;

/// How wide the page's column is at most, in logical pixels.
const PAGE_WIDTH: f32 = 860.0;

/// How long a reason a row has room for, in characters.
const PROBLEM_LENGTH: usize = 96;

/// What the extensions page remembers: the search and how far it is scrolled.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionsPage {
    /// What is searched for.
    pub search: Field,
    /// How far down the list is scrolled, in logical pixels.
    pub scroll: f32,
}

/// Whether `id` names a button of the extensions page's rows.
pub fn is_extensions(id: FocusId) -> bool {
    (ROW_BASE..EXTENSIONS_END).contains(&id.0)
}

/// The extensions the reader picks from: every one but those installed only as part of another.
pub fn pickable(extensions: &[ExtensionInfo]) -> impl Iterator<Item = &ExtensionInfo> {
    extensions.iter().filter(|extension| !extension.hidden)
}

/// `words` cut to `most` characters, with an ellipsis where it was cut.
fn shortened(words: &str, most: usize) -> String {
    if words.chars().count() <= most {
        return words.to_owned();
    }
    let kept: String = words.chars().take(most - 1).collect();
    format!("{}…", kept.trim_end())
}

/// What an extension is doing or has, in words, and the colour they are drawn in; nothing for one that is only available.
pub fn standing(extension: &ExtensionInfo) -> (String, Role) {
    if let Some(busy) = &extension.busy {
        return (format!("{busy}…"), Role::Muted);
    }
    if let Some(problem) = &extension.problem {
        return (shortened(problem, PROBLEM_LENGTH), Role::Danger);
    }
    match (&extension.server, extension.installed) {
        (Some(server), true) => (server.clone(), Role::Muted),
        (None, true) => ("No language server".to_owned(), Role::Subtle),
        (_, false) => (String::new(), Role::Subtle),
    }
}

/// The endings of the file names a language is written in, as `.rs .py`.
pub fn endings(extension: &ExtensionInfo) -> String {
    extension
        .suffixes
        .iter()
        .take(4)
        .map(|suffix| format!(".{suffix}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl WorkspaceState {
    /// The state after the server on this machine said which extensions there are.
    pub fn with_extensions(mut self, extensions: Vec<ExtensionInfo>) -> Self {
        crate::file_icon::learn_icons(&None, &extensions);
        self.extensions = extensions;
        self
    }

    /// The extensions the page lists, as the search narrows them: those installed first, then those available, each by name.
    fn listed_extensions(&self) -> Vec<&ExtensionInfo> {
        let needle = self.extensions_page.search.text().trim().to_lowercase();
        let mut listed: Vec<&ExtensionInfo> = pickable(&self.extensions)
            .filter(|extension| {
                needle.is_empty()
                    || extension.name.to_lowercase().contains(&needle)
                    || extension.description.to_lowercase().contains(&needle)
                    || extension
                        .suffixes
                        .iter()
                        .any(|suffix| suffix.contains(&needle))
            })
            .collect();
        listed.sort_by_key(|extension| (!extension.installed, extension.name.to_lowercase()));
        listed
    }

    /// The state with the extensions page showing, asking the server on this machine for the extensions there are.
    pub(crate) fn with_extensions_page(mut self) -> Self {
        self.page = crate::state::Page::Extensions;
        self.outbox.push(Outgoing::Extensions);
        self
    }

    /// The state after `edit` was made to the box the extensions are searched with.
    pub(crate) fn extensions_searched(&mut self, edit: Edit) {
        self.extensions_page.search.apply(edit);
        self.extensions_page.scroll = 0.0;
    }

    /// The state after the extensions page is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_extensions(mut self, delta: f32, room: f32) -> Self {
        self.extensions_page.scroll = (self.extensions_page.scroll + delta).clamp(0.0, room);
        self
    }

    /// The state after the reader chose the folder at `path` to install as an extension on the server on this machine.
    pub fn extension_picked(mut self, path: String) -> Self {
        self.outbox.push(Outgoing::InstallExtension { path });
        self
    }

    /// The state after the button of the `place`-th row, as `action` names it, was pressed: it installs the extension, or uninstalls it when it is installed.
    pub(crate) fn extensions_pressed(mut self, action: FocusId) -> Self {
        let place = (action.0 - ROW_BASE) as usize;
        let Some(extension) = self.listed_extensions().get(place).copied().cloned() else {
            return self;
        };
        if extension.busy.is_none() {
            self.outbox.push(if extension.installed {
                Outgoing::RemoveExtension { id: extension.id }
            } else {
                Outgoing::InstallExtensions {
                    ids: vec![extension.id],
                }
            });
        }
        self
    }
}

/// The title of the page and the button that installs an extension from a folder.
fn heading() -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            h(12),
            px(5),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(line("Extensions", 14, 1.4, Role::Text).styles([font_medium(), text_px(14)]))
        .child(
            Button::new("Install Extension…", INSTALL_EXTENSION)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        )
}

/// The box the reader types into to narrow the list.
fn search(state: &WorkspaceState) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            w_full(),
            h(10),
            px(3),
            flex_none(),
            rounded(Radius::Lg),
            bg(Role::Surface),
            border(),
            border_color(Role::Border),
        ])
        .child(glyph(IconName::Search, 4.0, Role::Muted))
        .child(
            Input::new(&state.extensions_page.search, EXTENSIONS_SEARCH)
                .variant(input::Variant::Ghost)
                .size(input::Size::Sm)
                .placeholder("Search extensions")
                .build(),
        )
}

/// The icon an extension's files are shown with, once it is installed and has one; a plain code file otherwise.
fn mark(extension: &ExtensionInfo) -> Node {
    let artwork = extension
        .icon
        .as_ref()
        .filter(|_| extension.installed)
        .map(|source| Svg::arrived(&format!("extension-{}", extension.id), source));
    match artwork {
        Some(svg) => {
            let (_, tone) = crate::file_icon::file_kind(&format!(
                "file.{}",
                extension.suffixes.first().cloned().unwrap_or_default()
            ));
            node()
                .styles([size(5), flex_none(), text_color(tone)])
                .artwork(svg)
        }
        None => glyph(IconName::FileCode, 5.0, Role::Subtle).styles([flex_none()]),
    }
}

/// The row of `extension`, the `place`-th the page lists: its icon, then its name and version over what it is for and how it stands, and the button that installs or uninstalls it.
fn row(extension: &ExtensionInfo, place: usize) -> Node {
    let action = FocusId(ROW_BASE + place as u64);
    let button = match (extension.busy.is_some(), extension.installed) {
        (true, _) => Button::new("Installing", action)
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .disabled(true)
            .build(),
        (false, true) => Button::new("Uninstall", action)
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .build(),
        (false, false) => Button::new("Install", action)
            .variant(Variant::Outline)
            .size(Size::Sm)
            .build(),
    };
    let (said, tone) = standing(extension);
    let mut name = node()
        .styles([flex(), items_center(), gap(2), min_w_px(0)])
        .child(line(extension.name.clone(), 14, 1.4, Role::Text).styles([font_medium()]));
    if !extension.version.is_empty() {
        name = name
            .child(line(extension.version.clone(), 12, 1.4, Role::Subtle).styles([font_mono()]));
    }
    if extension.local {
        name = name.child(line("your own", 12, 1.4, Role::Subtle));
    }
    let about = if extension.description.is_empty() {
        endings(extension)
    } else {
        format!("{}  {}", extension.description, endings(extension))
    };
    node()
        .styles([flex(), items_center(), gap(3), px(4), py(3), flex_none()])
        .child(mark(extension))
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_w_px(0), overflow_hidden()])
                .child(name)
                .child(line(about, 12, 1.4, Role::Muted))
                .children((!said.is_empty()).then(|| line(said, 12, 1.4, tone))),
        )
        .child(button)
}

/// A titled group of rows in one outlined box, or nothing when it has none.
fn group(title: &str, rows: Vec<Node>) -> Option<Node> {
    if rows.is_empty() {
        return None;
    }
    let last = rows.len() - 1;
    Some(
        node()
            .styles([flex(), flex_col(), gap(2), flex_none()])
            .child(line(title.to_owned(), 12, 1.4, Role::Subtle))
            .child(
                node()
                    .styles([
                        flex(),
                        flex_col(),
                        flex_none(),
                        rounded(Radius::Lg),
                        border(),
                        border_color(Role::Border),
                        bg(Role::Surface),
                        overflow_hidden(),
                    ])
                    .children(rows.into_iter().enumerate().map(|(at, row)| {
                        if at == last {
                            row
                        } else {
                            row.styles([border_b(), border_color(Role::Border)])
                        }
                    })),
            ),
    )
}

/// The extensions page for `state`.
pub fn extensions(state: &WorkspaceState) -> Node {
    let listed = state.listed_extensions();
    let mut installed = Vec::new();
    let mut available = Vec::new();
    for (place, extension) in listed.iter().enumerate() {
        let built = row(extension, place);
        if extension.installed {
            installed.push(built);
        } else {
            available.push(built);
        }
    }
    let mut column = node()
        .styles([
            flex(),
            flex_col(),
            gap(6),
            w_full(),
            max_w_px(PAGE_WIDTH),
            mx_auto(),
            px(5),
            pt(5),
            pb(8),
            flex_none(),
        ])
        .child(search(state));
    column = if state.extensions.is_empty() {
        column.child(line(
            "Asking the server which extensions there are…",
            13,
            1.4,
            Role::Subtle,
        ))
    } else if listed.is_empty() {
        column.child(line("No extension matches.", 13, 1.4, Role::Subtle))
    } else {
        column
            .children(group("Installed on this machine", installed))
            .children(group("Available", available))
    };
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(heading())
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(EXTENSIONS_SCROLL, state.extensions_page.scroll)
                .child(column),
        )
}

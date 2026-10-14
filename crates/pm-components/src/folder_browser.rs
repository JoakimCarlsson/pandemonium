//! The folder browser of the modal for adding a project: places on the left, the path as a trail
//! above, the folders as tiles, and the button that takes the folder being looked inside.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border_b, border_color, border_r, border_t, content_start, cursor_pointer,
    flex, flex_1, flex_col, flex_none, flex_wrap, font_medium, gap, h, h_px, items_center,
    justify_center, min_w_px, overflow_hidden, p, px, rounded, truncate, w_full, w_px,
};
use pm_ui::{Node, node};

use crate::Input;
use crate::adder::{
    ADDER_CONFIRM, ADDER_CRUMB_BASE, ADDER_ESCAPE, ADDER_FIELD, ADDER_PLACE_BASE, ADDER_ROW_BASE,
    Adder, status_line,
};
use crate::button::Variant;
use crate::input;
use crate::parts::{glyph, line, spacer};
use crate::state::MENU_SCROLL;
use crate::{Button, popover};

/// How wide the modal holding the browser is, in logical pixels.
pub const WIDTH: f32 = 880.0;

/// How tall the part of the browser between its bars is, in logical pixels.
const BODY_HEIGHT: f32 = 360.0;

/// How wide the column of places is, in logical pixels.
const PLACES_WIDTH: f32 = 176.0;

/// How wide a tile of a folder is, in logical pixels.
const TILE_WIDTH: f32 = 104.0;

/// How wide the box filtering the folders is, in logical pixels.
const FILTER_WIDTH: f32 = 220.0;

/// The places that are one press away: what they are called, where they are and their glyph.
pub const PLACES: [(&str, &str, IconName); 5] = [
    ("Home", "~", IconName::Folder),
    ("Desktop", "~/Desktop", IconName::Folder),
    ("Documents", "~/Documents", IconName::Folder),
    ("Downloads", "~/Downloads", IconName::Folder),
    ("Root", "/", IconName::Folder),
];

/// The folders that lead to `path`, nearest the root first: what each is called and where it is.
pub fn trail(path: &str) -> Vec<(String, String)> {
    let mut crumbs = vec![("/".to_owned(), "/".to_owned())];
    let mut at = String::new();
    for name in path.split('/').filter(|name| !name.is_empty()) {
        at = format!("{at}/{name}");
        crumbs.push((name.to_owned(), at.clone()));
    }
    crumbs
}

/// What the modal holds while the folders are showing.
pub fn view(open: &Adder, scrolled: f32) -> Vec<Node> {
    let cloning = open.clone_request.as_ref();
    let title = match cloning {
        Some(repository) => format!("Clone {} into", repository.name),
        None => "Open project folder".to_owned(),
    };
    let confirm = match cloning {
        Some(_) => "Clone here",
        None => "Open",
    };
    vec![
        node()
            .styles([
                flex(),
                items_center(),
                h(11),
                px(4),
                flex_none(),
                border_b(),
                border_color(Role::Border),
            ])
            .child(line(title, 13, 1.4, Role::Text)),
        bar(open),
        node()
            .styles([flex(), h_px(BODY_HEIGHT), flex_none(), overflow_hidden()])
            .child(places())
            .child(grid(open, scrolled)),
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                h(14),
                px(4),
                flex_none(),
                border_t(),
                border_color(Role::Border),
            ])
            .child(popover::key_hint("Esc", "Back"))
            .child(spacer())
            .child(
                Button::new("Cancel", ADDER_ESCAPE)
                    .variant(Variant::Ghost)
                    .build(),
            )
            .child(Button::new(confirm, ADDER_CONFIRM).build()),
    ]
}

/// The bar above the folders: the path as a trail that can be pressed anywhere along, and the box filtering the folders.
fn bar(open: &Adder) -> Node {
    let crumbs = open.directory.as_deref().map(trail).unwrap_or_default();
    let last = crumbs.len().saturating_sub(1);
    let mut trail = node().styles([flex(), items_center(), gap(0.5), flex_1(), min_w_px(0)]);
    for (index, (name, _)) in crumbs.iter().enumerate() {
        if index > 0 {
            trail = trail.child(glyph(IconName::ChevronRight, 3.0, Role::Subtle));
        }
        let tone = if index == last {
            Role::Text
        } else {
            Role::Muted
        };
        let mut label = line(name.clone(), 13, 1.4, tone);
        if index == last {
            label = label.styles([font_medium()]);
        }
        trail = trail.child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    h(7),
                    px(1.5),
                    rounded(Radius::Md),
                    cursor_pointer(),
                    bg(Role::SurfaceHover).hover(),
                ])
                .child(label)
                .focusable(FocusId(ADDER_CRUMB_BASE + index as u64)),
        );
    }
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(12),
            px(3),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(trail)
        .child(
            node().styles([w_px(FILTER_WIDTH), flex_none()]).child(
                Input::new(&open.field, ADDER_FIELD)
                    .size(input::Size::Sm)
                    .placeholder("Filter folders")
                    .build(),
            ),
        )
}

/// The column of places that are one press away.
fn places() -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(0.5),
            w_px(PLACES_WIDTH),
            p(2),
            border_r(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), h(8), px(2)])
                .child(line("Places", 12, 1.4, Role::Subtle)),
        )
        .children(PLACES.iter().enumerate().map(|(index, (name, _, icon))| {
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(2.5),
                    h(8),
                    px(2),
                    rounded(Radius::Md),
                    cursor_pointer(),
                    bg(Role::SurfaceHover).hover(),
                ])
                .child(glyph(*icon, 4.0, Role::Muted))
                .child(line(*name, 13, 1.4, Role::Text))
                .focusable(FocusId(ADDER_PLACE_BASE + index as u64))
        }))
}

/// The folders inside the one being looked into, as tiles that wrap, scrolled by `scrolled`.
fn grid(open: &Adder, scrolled: f32) -> Node {
    let mut grid = node()
        .styles([
            flex(),
            flex_wrap(),
            content_start(),
            gap(1),
            p(3),
            flex_1(),
            min_w_px(0),
            h_px(BODY_HEIGHT),
            overflow_hidden(),
        ])
        .scroll_from_start(MENU_SCROLL, scrolled);
    grid = grid.children(
        open.entries()
            .into_iter()
            .enumerate()
            .map(|(index, found)| tile(&found.name, index == open.selected, index)),
    );
    if let Some(status) = open.status() {
        grid = grid.child(node().styles([w_full()]).child(status_line(status)));
    }
    grid
}

/// One folder: its glyph large over its name, highlighted while it is the one the keys act on.
fn tile(name: &str, selected: bool, index: usize) -> Node {
    let mut styles = vec![
        flex(),
        flex_col(),
        items_center(),
        gap(1.5),
        w_px(TILE_WIDTH),
        p(2),
        flex_none(),
        rounded(Radius::Lg),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if selected {
        styles.push(bg(Role::SurfaceSelected));
    }
    node()
        .styles(styles)
        .child(glyph(IconName::Folder, 12.0, Role::Accent))
        .child(
            node()
                .styles([flex(), justify_center(), w_full(), overflow_hidden()])
                .child(line(name, 12, 1.4, Role::Text).styles([truncate()])),
        )
        .focusable(FocusId(ADDER_ROW_BASE + index as u64))
}

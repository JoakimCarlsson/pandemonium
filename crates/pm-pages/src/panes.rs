//! The main area of a page cut into panes: each pane's strip of tabs above the tab it shows, the handles between panes, and, while a tab is dragged, where it would land.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Length, Radius, Role, Sides, absolute, bg, border, border_b, border_color, border_l, border_t,
    flex, flex_1, flex_col, flex_none, gap, h, h_full, h_px, inset_length, items_center, items_end,
    justify_center, mb, min_h, min_w_px, opacity, overflow_hidden, px, rounded, shadow_md, size,
    text_color, w_px, z,
};
use pm_text::Shaper;
use pm_ui::{Node, node};

use pm_components::Menu;
use pm_components::Size;
use pm_components::browser::{self, BROWSER_CLOSE, TAB_BROWSER};
use pm_components::button::Variant;
use pm_components::excerpts::{self, EXCERPTS_CLOSE, Source, TAB_EXCERPTS};
use pm_components::file_icon::file_icon;
use pm_components::open_files::{FILE_CLOSE_BASE, FILE_TAB_BASE, Viewer};
use pm_components::panes::{
    Area, Axis, Group, Item, Layout, PANE_BASE, PANE_MENU, PANE_PUT_BASE, Placed, SPLIT_DOWN,
    SPLIT_LEFT, SPLIT_RIGHT, SPLIT_UP,
};
use pm_components::parts::{glyph, line, spacer};
use pm_components::sash::Sash;
use pm_components::state::{Page, TAB_CHAT, TAB_DIFF, WorkspaceState};
use pm_components::title_bar::TITLE_BAR_HEIGHT;
use pm_components::update_dialog::{TAB_UPDATE, UPDATE_CLOSE};
use pm_components::{
    Button, Changes, Conflict, chat, diff, input, overview, terminal, viewer, welcome,
};

/// How much of a window's height the conversation keeps for something other than the room above the prompt box, in logical pixels.
const CHAT_CHROME: f32 = 230.0;

/// How tall the page for starting work is, in logical pixels.
const WELCOME_HEIGHT: f32 = 265.0;

/// How far down the page for starting work its prompt box starts, from the top of the page's own content, in logical pixels.
const WELCOME_ABOVE: f32 = 127.0;

/// The look of a tab: rounded at the top, filled while it is the one its pane shows.
fn tab_styles(shown: bool) -> Vec<pm_style::Utility> {
    let tone = if shown { Role::Text } else { Role::Muted };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(10),
        px(3),
        pm_style::rounded_t(8),
        pm_style::cursor_pointer(),
        text_color(tone),
    ];
    if shown {
        styles.push(bg(Role::Surface));
    } else {
        styles.push(bg(Role::SurfaceHover).hover());
    }
    styles
}

/// One tab of a pane's strip: a glyph and a label, filled while it is the one shown.
fn tab(icon: IconName, label: &str, id: FocusId, shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    node()
        .styles(tab_styles(shown))
        .child(glyph(icon, 3.5, tone))
        .child(line(label, 13, 1.4, tone))
        .focusable(id)
}

/// The tab of the open file at `index`: its icon and name, set slanted while the file is only being looked at, a dot while it has changes that are not saved, and a button that closes it.
fn viewer_tab(viewer: &Viewer, index: usize, shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    let name = viewer.path.rsplit('/').next().unwrap_or(&viewer.path);
    let label = if viewer.head {
        format!("{name} (HEAD)")
    } else {
        name.to_owned()
    };
    let mut name_line = line(&label, 13, 1.4, tone);
    if viewer.preview {
        name_line = name_line.styles([pm_style::italic()]);
    }
    let mut tab = node()
        .styles(tab_styles(shown))
        .child(file_icon(&viewer.path, 3.5))
        .child(name_line);
    if viewer.is_dirty() {
        tab = tab.child(node().styles([
            pm_style::size(2.0),
            pm_style::rounded(Radius::Xs),
            bg(Role::Muted),
        ]));
    }
    tab.child(
        Button::icon(
            if viewer.pinned {
                IconName::Pin
            } else {
                IconName::Close
            },
            FocusId(FILE_CLOSE_BASE + index as u64),
        )
        .size(pm_components::Size::Sm)
        .build(),
    )
    .focusable(FocusId(FILE_TAB_BASE + index as u64))
}

/// The tab of the excerpts: a glyph and what they are of, with a button that closes them.
fn excerpts_tab(state: &WorkspaceState, shown: bool) -> Option<Node> {
    let excerpts = state.excerpts.as_ref()?;
    let tone = if shown { Role::Text } else { Role::Muted };
    Some(
        node()
            .styles(tab_styles(shown))
            .child(glyph(excerpts.icon(), 3.5, tone))
            .child(line(excerpts.title(), 13, 1.4, tone))
            .child(
                Button::icon(IconName::Close, EXCERPTS_CLOSE)
                    .size(pm_components::Size::Sm)
                    .build(),
            )
            .focusable(TAB_EXCERPTS),
    )
}

/// The tab of the browser in a pane's strip, filled while `shown`.
fn browser_tab(state: &WorkspaceState, shown: bool) -> Option<Node> {
    let browser = state.browser.as_ref()?;
    let tone = if shown { Role::Text } else { Role::Muted };
    let title = if browser.title.is_empty() {
        "Browser"
    } else {
        browser.title.as_str()
    };
    Some(
        node()
            .styles(tab_styles(shown))
            .child(glyph(IconName::Globe, 3.5, tone))
            .child(line(title, 13, 1.4, tone))
            .child(
                Button::icon(IconName::Close, BROWSER_CLOSE)
                    .size(pm_components::Size::Sm)
                    .build(),
            )
            .focusable(TAB_BROWSER),
    )
}

/// The tab of the release notes, filled while `shown`.
fn update_tab(shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    node()
        .styles(tab_styles(shown))
        .child(glyph(IconName::ArrowDown, 3.5, tone))
        .child(line("Update", 13, 1.4, tone))
        .child(
            Button::icon(IconName::Close, UPDATE_CLOSE)
                .size(pm_components::Size::Sm)
                .build(),
        )
        .focusable(TAB_UPDATE)
}

/// A square button in the strip of tabs holding only `icon`, filled while `on`, saying `hint` when the pointer rests on it.
fn strip_button(icon: IconName, id: FocusId, on: bool, hint: &str) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        justify_center(),
        size(8.0),
        mb(1),
        rounded(Radius::Lg),
        pm_style::cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if on {
        styles.push(bg(Role::SurfaceSelected));
    }
    let tone = if on { Role::Text } else { Role::Muted };
    node()
        .styles(styles)
        .child(glyph(icon, 3.5, tone))
        .tooltip(hint)
        .focusable(id)
}

/// The buttons at the end of the strip of tabs: the one that opens the menu of panes.
fn strip_buttons(state: &WorkspaceState) -> Node {
    node()
        .styles([flex(), items_end(), gap(1)])
        .child(strip_button(
            IconName::SquareSplit,
            PANE_MENU,
            state.pane_menu.is_some(),
            "Split pane",
        ))
}

/// The menu of panes, opened with its corner at `at`: the pane being worked in split with an empty pane on each side.
pub(crate) fn pane_menu(at: (f32, f32)) -> Node {
    Menu::new(at.0, at.1)
        .item("Split pane right", None, SPLIT_RIGHT, false)
        .item("Split pane left", None, SPLIT_LEFT, false)
        .item("Split pane up", None, SPLIT_UP, false)
        .item("Split pane down", None, SPLIT_DOWN, false)
        .build()
}

/// The glyph and label of the tab of the page itself.
fn page_label(state: &WorkspaceState) -> (IconName, String) {
    match (state.page, state.open()) {
        (Page::Session(_), Some(session)) => {
            (IconName::Sparkles, state.agent_look(session.agent).name)
        }
        (Page::Project(_), _) => (IconName::Folder, "Overview".to_owned()),
        _ => (IconName::Plus, "New Session".to_owned()),
    }
}

/// The open file `item` names, and its position.
fn viewer_of<'a>(state: &'a WorkspaceState, item: &Item) -> Option<(usize, &'a Viewer)> {
    let Item::File { path, head } = item else {
        return None;
    };
    state
        .git
        .viewers
        .iter()
        .enumerate()
        .find(|(_, viewer)| viewer.path == *path && viewer.head == *head)
}

/// The tab of `item` in a pane's strip, filled while `shown`.
fn item_tab(state: &WorkspaceState, item: &Item, shown: bool) -> Option<Node> {
    match item {
        Item::Page => {
            let (icon, label) = page_label(state);
            Some(tab(icon, &label, TAB_CHAT, shown))
        }
        Item::Diff => Some(tab(IconName::GitCompare, "Diff", TAB_DIFF, shown)),
        Item::Excerpts => excerpts_tab(state, shown),
        Item::Browser => browser_tab(state, shown),
        Item::Update => Some(update_tab(shown)),
        Item::File { .. } => {
            viewer_of(state, item).map(|(index, viewer)| viewer_tab(viewer, index, shown))
        }
    }
}

/// The strip above a pane: the tab of everything in `group`, and, in the pane at the top right, `toggle`, the button for the terminal.
fn strip(state: &WorkspaceState, group: &Group, toggle: bool) -> Node {
    let strip = node()
        .styles([
            flex(),
            items_end(),
            gap(1),
            h(12),
            px(3),
            flex_none(),
            min_w_px(0),
            overflow_hidden(),
            border_b(),
            border_color(Role::Border),
        ])
        .children(
            group
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| item_tab(state, item, index == group.active)),
        );
    if toggle {
        strip.child(spacer()).child(strip_buttons(state))
    } else {
        strip
    }
}

/// The page for starting work, its prompt box `width` logical pixels across in a view `height` tall.
pub(crate) fn starting_page(
    state: &WorkspaceState,
    width: f32,
    height: f32,
    shaper: &mut Shaper,
) -> Node {
    welcome::welcome(
        state,
        ((height - WELCOME_HEIGHT) / 2.0 + WELCOME_ABOVE).max(0.0),
        input::inner_width((width - 48.0).min(680.0)),
        shaper,
    )
}

/// What `item` shows in a pane `area` big, its views given `tall` logical pixels of height as they count it.
fn item_view(
    state: &WorkspaceState,
    item: &Item,
    area: Area,
    tall: f32,
    shaper: &mut Shaper,
) -> Node {
    let width = area.width;
    match (item, state.page, state.open()) {
        (Item::Page, Page::Session(_), Some(session)) => {
            chat::chat(state, session, width, (tall - CHAT_CHROME).max(0.0), shaper)
        }
        (Item::Page, Page::Project(project), _) => overview::overview(state, project),
        (Item::Page, _, _) => starting_page(state, width, area.height, shaper),
        (Item::Diff, _, _) => match state.review_session() {
            Some(session) => diff::diff(state, session, width, tall),
            None => node(),
        },
        (Item::Excerpts, _, _) => excerpts_item(state, width, tall),
        (Item::Browser, _, _) => {
            browser::browser_view(state, width, area.height - pm_components::panes::TABS)
        }
        (Item::Update, _, _) => match state.settings_page.release.as_ref() {
            Some(update) => pm_components::update_dialog::page(state, update, width, shaper),
            None => node(),
        },
        (Item::File { .. }, _, _) => match viewer_of(state, item) {
            Some((index, _)) => viewer::file_view(state, index, width, tall, shaper),
            None => node(),
        },
    }
}

/// The excerpts shown in a pane `width` by `height` logical pixels: the resolver for a file in conflict, the view of changes for what a change set or a timeline holds, and the lines around the hits for a search.
fn excerpts_item(state: &WorkspaceState, width: f32, height: f32) -> Node {
    match state.excerpts.as_ref().map(|excerpts| &excerpts.source) {
        Some(Source::Conflict(_)) => Conflict::new(state, width, height).build(),
        Some(Source::Timeline { .. } | Source::Revision { .. }) => {
            Changes::new(state, height).build()
        }
        _ => excerpts::excerpts_view(state, height),
    }
}

/// What an empty pane at `index` offers: the page, the diff and the browser brought into it, the files shown to open one in it, and closing it.
fn empty_pane(state: &WorkspaceState, index: usize) -> Node {
    let put = |kind: u64| FocusId(PANE_PUT_BASE + index as u64 * 5 + kind);
    let (icon, label) = page_label(state);
    let mut buttons = node()
        .styles([flex(), flex_col(), items_center(), gap(2)])
        .child(
            Button::new(label, put(0))
                .with_icon(icon)
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build(),
        );
    if state
        .open()
        .is_some_and(|session| !session.changes.is_empty())
    {
        buttons = buttons.child(
            Button::new("Diff", put(1))
                .with_icon(IconName::GitCompare)
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build(),
        );
    }
    let buttons = buttons
        .child(
            Button::new("Browser", put(4))
                .with_icon(IconName::Globe)
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build(),
        )
        .child(
            Button::new("Open a file", put(2))
                .with_icon(IconName::File)
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build(),
        )
        .child(
            Button::new("Close pane", put(3))
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        );
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            items_center(),
            justify_center(),
            gap(4),
            min_h(0),
        ])
        .child(line(
            "Bring something into this pane, or drag a tab here.",
            13,
            1.4,
            Role::Muted,
        ))
        .child(buttons)
}

/// The pane `placed`: its strip of tabs over the tab it shows, the button for the terminal in its strip when `toggle`, its views given `extra` logical pixels of height beyond its own.
fn pane(
    state: &WorkspaceState,
    placed: &Placed,
    toggle: bool,
    extra: f32,
    shaper: &mut Shaper,
) -> Node {
    let group = placed.group;
    let view = match group.items.get(group.active) {
        Some(item) => item_view(state, item, placed.area, placed.area.height + extra, shaper),
        None => empty_pane(state, placed.index),
    };
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_w_px(0),
            min_h(0),
            overflow_hidden(),
        ])
        .child(strip(state, group, toggle))
        .child(view)
        .focusable(FocusId(PANE_BASE + placed.index as u64))
}

/// The panes of `layout`, laid out in `area` of the panes' whole area `whole`, the one counted next being `placed[*next]`, their views given `extra` logical pixels of height beyond their own.
fn laid(
    state: &WorkspaceState,
    layout: &Layout,
    placed: &[Placed],
    next: &mut usize,
    (area, whole, extra): (Area, Area, f32),
    shaper: &mut Shaper,
) -> Node {
    match layout {
        Layout::Group(_) => {
            let Some(here) = placed.get(*next) else {
                return node();
            };
            *next += 1;
            let toggle = here.area.y <= whole.y + 0.5
                && here.area.x + here.area.width >= whole.x + whole.width - 0.5;
            pane(state, here, toggle, extra, shaper)
        }
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let (one_area, two_area) = area.halves(*axis, *ratio);
            let one = laid(state, first, placed, next, (one_area, whole, extra), shaper);
            let two = laid(
                state,
                second,
                placed,
                next,
                (two_area, whole, extra),
                shaper,
            );
            let (direction, sized, edge) = match axis {
                Axis::Row => (flex(), w_px(one_area.width), border_l()),
                Axis::Column => (flex_col(), h_px(one_area.height), border_t()),
            };
            node()
                .styles([
                    flex(),
                    direction,
                    flex_1(),
                    min_w_px(0),
                    min_h(0),
                    overflow_hidden(),
                ])
                .child(
                    node()
                        .styles([flex(), flex_none(), min_w_px(0), min_h(0), sized])
                        .child(one),
                )
                .child(
                    node()
                        .styles([
                            flex(),
                            flex_1(),
                            min_w_px(0),
                            min_h(0),
                            edge,
                            border_color(Role::Border),
                        ])
                        .child(two),
                )
        }
    }
}

/// The highlight over the part of a pane a dragged tab would take, and the tab itself under the pointer.
fn drag_marks(state: &WorkspaceState, placed: &[Placed]) -> Vec<Node> {
    let Some(drag) = state.pane_drag.as_ref() else {
        return Vec::new();
    };
    let mut marks = Vec::new();
    if let Some((index, side)) = drag.over
        && let Some(here) = placed.get(index)
    {
        let zone = here.area.of_side(side);
        marks.push(node().styles([
            absolute(),
            z(50),
            inset_length(Sides::Left, Length::Px(zone.x + 4.0)),
            inset_length(Sides::Top, Length::Px(zone.y + 4.0)),
            w_px((zone.width - 8.0).max(0.0)),
            h_px((zone.height - 8.0).max(0.0)),
            rounded(Radius::Lg),
            bg(Role::Accent),
            opacity(18),
        ]));
    }
    if let Some(ghost) = item_tab(state, &drag.item, true) {
        marks.push(
            node()
                .styles([
                    absolute(),
                    z(60),
                    inset_length(Sides::Left, Length::Px(drag.at.0 + 8.0)),
                    inset_length(Sides::Top, Length::Px(drag.at.1 + 8.0)),
                    rounded(Radius::Md),
                    border(),
                    border_color(Role::Border),
                    shadow_md(),
                ])
                .child(ghost),
        );
    }
    marks
}

/// The main area of a page in a window `window` logical pixels wide and tall: its panes over the terminal dock while the dock shows, and, to lay over the whole window, the handles between the panes and the marks of a tab being dragged.
pub(crate) fn main_area(
    state: &WorkspaceState,
    window: (f32, f32),
    shaper: &mut Shaper,
) -> (Node, Vec<Node>) {
    let area = state.pane_area(window);
    let panes = state.panes();
    let placed = panes.layout.placed(area);
    let page = window.1 - TITLE_BAR_HEIGHT;
    let extra = page - area.height;
    let mut next = 0;
    let body = laid(
        state,
        &panes.layout,
        &placed,
        &mut next,
        (area, area, extra),
        shaper,
    );
    let mut column = node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), h_full()])
        .child(
            node()
                .styles([flex(), flex_1(), min_w_px(0), min_h(0)])
                .child(body),
        );
    if let Some(session) = state.docked().filter(|_| state.docking()) {
        column = column.child(terminal::terminal(state, session, area.width, page, shaper));
    }
    let mut over: Vec<Node> = panes
        .layout
        .seams(area)
        .into_iter()
        .map(|seam| match seam.axis {
            Axis::Row => Sash::column(seam.id).at(seam.at, seam.length).build(),
            Axis::Column => Sash::row(seam.id).at(seam.at, seam.length).build(),
        })
        .collect();
    over.extend(drag_marks(state, &placed));
    (column, over)
}

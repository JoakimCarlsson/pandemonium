//! The main area of a page cut into panes: each pane's strip of tabs above the tab it shows, the handles between panes, and, while a tab is dragged, where it would land.

use pm_components::areas::WindowEvent;
use pm_components::areas::{
    BrowserEvent, ChatEvent, DialogEvent, DiffEvent, EditorEvent, SearchEvent,
};
use pm_components::theme::Indicated;
use pm_components::theme::ShadowLayers;

use pm_components::events::Interactive;
use pm_components::events::{Event, OnContext, send};
use pm_components::pointing::Over;
use pm_components::target::Target;
use pm_components::theme;
use pm_components::theme::sp;
use pm_components::theme::{Radius, Role};
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{
    BoxAlignment, CursorIcon, Modifier, Shape, ShapeModifier, remember_mutable_interaction_source,
};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use crate::observed::region;
use pm_components::Menu;
use pm_components::Size;
use pm_components::browser;
use pm_components::button::Variant;
use pm_components::excerpts::{self, Source};
use pm_components::file_icon::file_icon;
use pm_components::open_files::Viewer;
use pm_components::panes::{Area, Axis, Group, Item, Layout, Placed, Side};
use pm_components::parts::{glyph, line, spacer};
use pm_components::sash::Sash;
use pm_components::state::{Page, WorkspaceState};
use pm_components::title_bar::TITLE_BAR_HEIGHT;

use pm_components::{Button, Changes, Conflict, chat, diff, overview, terminal, viewer, welcome};

/// How much of a window's height the conversation keeps for something other than the room above the prompt box, in logical pixels.
const CHAT_CHROME: f32 = 230.0;

/// How tall the page for starting work is, in logical pixels.
const WELCOME_HEIGHT: f32 = 265.0;

/// How far down the page for starting work its prompt box starts, from the top of the page's own content, in logical pixels.
const WELCOME_ABOVE: f32 = 127.0;

/// The corner radius of the top of a tab, in logical pixels.
const TAB_CORNER: f32 = 8.0;

/// The box of a tab answering to `id`: rounded at the top, filled while it is the one its pane shows and while the pointer is on it otherwise, with its padding; as wide as what it holds, running past the end of a strip too narrow for it. A secondary press runs `on_context` when the tab has a menu.
#[track_caller]
fn tab_modifier(shown: bool, id: Target, on_context: Option<OnContext>) -> Modifier {
    let surface = pm_components::theme::color(Role::Surface);
    let hover = pm_components::theme::color(Role::SurfaceHover);
    let over = remember_mutable_interaction_source();
    let hovered = over.collect_is_hovered_as_state();
    let fill = if shown {
        Some(surface)
    } else {
        hovered.get().then_some(hover)
    };
    let own = Modifier::new()
        .wrap_content_width(BoxAlignment::CenterStart, true)
        .height(theme::sp(10))
        .hoverable(&over);
    let own = match fill {
        Some(fill) => own.background_shape(
            fill,
            Shape::rounded_corners(TAB_CORNER, TAB_CORNER, 0.0, 0.0),
        ),
        None => own,
    };
    own.cursor(CursorIcon::Pointer)
        .chain(match on_context {
            Some(on_context) => Modifier::new().control_menu(
                id,
                {
                    let pressed = send(Event::pressing(id));
                    move |_| pressed()
                },
                move |at| on_context(at),
            ),
            None => Modifier::new().control(id, send(Event::pressing(id))),
        })
        .padding_xy(theme::sp(3), 0.0)
}

/// The tone of the text and glyph of a tab, brighter while `shown`.
fn tab_tone(shown: bool) -> Role {
    if shown { Role::Text } else { Role::Muted }
}

/// Emits one tab of a pane's strip answering to `id`: a glyph and a label, filled while it is the one shown, then `extra`.
#[track_caller]
fn tab_with(icon: Icon, label: &str, id: Target, shown: bool, extra: impl FnOnce()) {
    let tone = tab_tone(shown);
    row_arranged(
        tab_modifier(shown, id, None),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            glyph(icon, 3.5, tone);
            line(label, 13, 1.4, tone);
            extra();
        },
    );
}

/// Emits one tab of a pane's strip: a glyph and a label, filled while it is the one shown.
#[track_caller]
fn tab(icon: Icon, label: &str, id: Target, shown: bool) {
    tab_with(icon, label, id, shown, || {});
}

/// Emits the small button closing a tab, answering to `id`, showing `icon`.
#[track_caller]
fn close_button(icon: Icon, id: Target) {
    Button::icon(icon, id).size(Size::Sm).build();
}

/// Emits the tab of the open file at `index`: its icon and name, set slanted while the file is only being looked at, a dot while it has changes that are not saved, and a button that closes it.
fn viewer_tab(viewer: &Viewer, index: usize, shown: bool) {
    let tone = tab_tone(shown);
    let name = viewer.path.rsplit('/').next().unwrap_or(&viewer.path);
    let label_text = if viewer.head {
        format!("{name} (HEAD)")
    } else {
        name.to_owned()
    };
    let kind = theme::text_style(13, 1.4)
        .color(theme::color(tone))
        .max_lines(1);
    let kind = if viewer.preview { kind.italic() } else { kind };
    row_arranged(
        tab_modifier(shown, Target::Editor(EditorEvent::FileTab(index)), {
            let operations = pm_components::events::operations();
            Some(Rc::new(move |at| operations.open_tab_menu(index, at)))
        }),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            file_icon(&viewer.path, 3.5);
            rustle::basic_text(
                Modifier::new(),
                label_text,
                kind,
                rustle::TextOverflow::Ellipsis,
            );
            if viewer.is_dirty() {
                rustle::spacer(
                    Modifier::new()
                        .size(theme::sp(2.0), theme::sp(2.0))
                        .background_rounded(theme::color(Role::Muted), Radius::Xs.px()),
                );
            }
            close_button(
                if viewer.pinned {
                    phosphor::PUSH_PIN
                } else {
                    phosphor::X
                },
                Target::Editor(EditorEvent::FileClose(index)),
            );
        },
    );
}

/// Emits the tab of the excerpts, when there are some: a glyph and what they are of, with a button that closes them.
fn excerpts_tab(state: &WorkspaceState, shown: bool) {
    let Some(excerpts) = state.excerpts.as_ref() else {
        return;
    };
    tab_with(
        excerpts.icon(),
        &excerpts.title(),
        Target::Search(SearchEvent::TabExcerpts),
        shown,
        || {
            close_button(phosphor::X, Target::Search(SearchEvent::ExcerptsClose));
        },
    );
}

/// Emits the tab of the browser in a pane's strip, when there is a browser, filled while `shown`.
#[track_caller]
fn browser_tab(state: &WorkspaceState, shown: bool) {
    let Some(browser) = state.browser.as_ref() else {
        return;
    };
    let title = if browser.title.is_empty() {
        "Browser"
    } else {
        browser.title.as_str()
    };
    tab_with(
        phosphor::GLOBE,
        title,
        Target::Browser(BrowserEvent::TabBrowser),
        shown,
        || {
            close_button(phosphor::X, Target::Browser(BrowserEvent::BrowserClose));
        },
    );
}

/// Emits the tab of the release notes, filled while `shown`.
fn update_tab(shown: bool) {
    tab_with(
        phosphor::ARROW_DOWN,
        "Update",
        Target::Dialog(DialogEvent::TabUpdate),
        shown,
        || {
            close_button(phosphor::X, Target::Dialog(DialogEvent::UpdateClose));
        },
    );
}

/// Emits a square button in the strip of tabs holding only `icon`, answering to `id`, filled while `on`, with `dropdown`, the menu it opens while it is open, declared in its box to hang from it, saying `hint` when the pointer rests on it and running `on_click` when clicked.
fn strip_button(
    icon: Icon,
    id: Target,
    (on, dropdown): (bool, Option<Menu>),
    hint: &str,
    on_click: impl Fn() + 'static,
) {
    let looks = theme::InteractionTones::new()
        .hover(Role::SurfaceHover)
        .radius(Radius::Lg);
    let looks = if on {
        looks.idle(Role::SurfaceSelected)
    } else {
        looks
    };
    let tone = if on { Role::Text } else { Role::Muted };
    pm_components::tooltip::with_tooltip(hint, || {
        row_arranged(
            Modifier::new()
                .padding_each(0.0, 0.0, 0.0, theme::sp(1))
                .size(theme::sp(8.0), theme::sp(8.0))
                .indicated(looks, theme::Focused::Rustle)
                .cursor(CursorIcon::Pointer)
                .control(id, on_click),
            Arrangement::Center,
            Alignment::Center,
            || {
                glyph(icon, 3.5, tone);
                if let Some(menu) = dropdown {
                    menu.build();
                }
            },
        );
    });
}

/// Emits the buttons at the end of the strip of tabs: the one that opens the menu of panes.
fn strip_buttons(state: &WorkspaceState) {
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(1)),
        Alignment::End,
        || {
            strip_button(
                phosphor::SQUARE_SPLIT_HORIZONTAL,
                Target::Window(WindowEvent::PaneMenu),
                (state.pane_menu, state.pane_menu.then(pane_menu)),
                "Split pane",
                {
                    let operations = pm_components::events::operations();
                    move || operations.open_pane_menu()
                },
            );
        },
    );
}

/// The menu of panes, hanging from the button it is emitted in: the pane being worked in split with an empty pane on each side.
fn pane_menu() -> Menu {
    Menu::dropdown()
        .item(
            "Split pane right",
            None,
            Target::Window(WindowEvent::SplitRight),
            false,
        )
        .item(
            "Split pane left",
            None,
            Target::Window(WindowEvent::SplitLeft),
            false,
        )
        .item(
            "Split pane up",
            None,
            Target::Window(WindowEvent::SplitUp),
            false,
        )
        .item(
            "Split pane down",
            None,
            Target::Window(WindowEvent::SplitDown),
            false,
        )
}

/// The glyph and label of the tab of the page itself.
fn page_label(state: &WorkspaceState) -> (Icon, String) {
    match (state.page, state.open()) {
        (Page::Session(_), Some(session)) => {
            (phosphor::SPARKLE, state.agent_look(session.agent).name)
        }
        (Page::Project(_), _) => (phosphor::FOLDER, "Overview".to_owned()),
        (Page::Chat, _) => (phosphor::CHAT_CIRCLE, "New Chat".to_owned()),
        _ => (phosphor::PLUS, "New Session".to_owned()),
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

/// Whether `item` has a tab to show: the excerpts, the browser and an open file only while they exist.
fn has_tab(state: &WorkspaceState, item: &Item) -> bool {
    match item {
        Item::Page | Item::Diff | Item::Update => true,
        Item::Excerpts => state.excerpts.is_some(),
        Item::Browser => state.browser.is_some(),
        Item::File { .. } => viewer_of(state, item).is_some(),
    }
}

/// Emits the tab of `item` in a pane's strip, when it has one, filled while `shown`.
#[track_caller]
fn item_tab(state: &WorkspaceState, item: &Item, shown: bool) {
    match item {
        Item::Page => {
            let (icon, label) = page_label(state);
            tab(icon, &label, Target::Chat(ChatEvent::TabChat), shown);
        }
        Item::Diff => tab(
            phosphor::GIT_DIFF,
            "Diff",
            Target::Diff(DiffEvent::TabDiff),
            shown,
        ),
        Item::Excerpts => excerpts_tab(state, shown),
        Item::Browser => browser_tab(state, shown),
        Item::Update => update_tab(shown),
        Item::File { .. } => {
            if let Some((index, viewer)) = viewer_of(state, item) {
                viewer_tab(viewer, index, shown);
            }
        }
    }
}

/// Emits the strip above a pane: the tab of everything in `group`, and, in the pane at the top right, `toggle`, the button for the terminal.
#[track_caller]
fn strip(state: &WorkspaceState, group: &Group, toggle: bool) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(12))
            .clip()
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(sp(1)),
        Alignment::End,
        || {
            for (index, item) in group.items.iter().enumerate() {
                rustle::key(index, || item_tab(state, item, index == group.active));
            }
            if toggle {
                spacer();
                strip_buttons(state);
            }
        },
    );
}

/// Emits the page for starting work in a view `height` tall.
#[track_caller]
pub(crate) fn starting_page(state: &WorkspaceState, height: f32) {
    welcome::welcome(
        state,
        ((height - WELCOME_HEIGHT) / 2.0 + WELCOME_ABOVE).max(0.0),
    );
}

/// Emits what `item` shows in a pane `area` big, its views given `tall` logical pixels of height as they count it.
fn item_view(state: &WorkspaceState, item: &Item, area: Area, tall: f32) {
    let width = area.width;
    match (item, state.page, state.open()) {
        (Item::Page, Page::Session(_), Some(session)) => {
            chat::chat(state, session, width, (tall - CHAT_CHROME).max(0.0));
        }
        (Item::Page, Page::Project(project), _) => overview::overview(state, project),
        (Item::Page, _, _) => starting_page(state, area.height),
        (Item::Diff, _, _) => {
            if let Some(session) = state.review_session() {
                diff::diff(state, session, width);
            }
        }
        (Item::Excerpts, _, _) => excerpts_item(state, width),
        (Item::Browser, _, _) => {
            browser::browser_view(state, width);
        }
        (Item::Update, _, _) => {
            if let Some(update) = state.settings_page.release.as_ref() {
                pm_components::update_dialog::page(state, update, width);
            }
        }
        (Item::File { .. }, _, _) => {
            if let Some((index, _)) = viewer_of(state, item) {
                viewer::file_view(state, index, width, tall);
            }
        }
    }
}

/// Emits the excerpts shown in a pane `width` logical pixels wide: the resolver for a file in conflict, the view of changes for what a change set or a timeline holds, and the lines around the hits for a search.
fn excerpts_item(state: &WorkspaceState, width: f32) {
    match state.excerpts.as_ref().map(|excerpts| &excerpts.source) {
        Some(Source::Conflict(_)) => Conflict::new(state, width).build(),
        Some(Source::Timeline { .. } | Source::Revision { .. }) => Changes::new(state).build(),
        _ => excerpts::excerpts_view(state),
    }
}

/// Emits a secondary button of an empty pane saying `words` beside `icon`, answering to `id`.
#[track_caller]
fn put_button(words: &str, icon: Icon, id: Target) {
    Button::new(words, id)
        .with_icon(icon)
        .variant(Variant::Secondary)
        .size(Size::Sm)
        .build();
}

/// Emits what an empty pane at `index` offers: the page, the diff and the browser brought into it, the files shown to open one in it, and closing it.
fn empty_pane(state: &WorkspaceState, index: usize) {
    let put = |put: usize| Target::Window(WindowEvent::PanePut { pane: index, put });
    let (icon, label_text) = page_label(state);
    let changed = state
        .open()
        .is_some_and(|session| !session.changes.is_empty());
    column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::SpacedByAligned(sp(4), Alignment::Center),
        Alignment::Center,
        || {
            line(
                "Bring something into this pane, or drag a tab here.",
                13,
                1.4,
                Role::Muted,
            );
            column_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    put_button(&label_text, icon, put(0));
                    if changed {
                        put_button("Diff", phosphor::GIT_DIFF, put(1));
                    }
                    put_button("Browser", phosphor::GLOBE, put(4));
                    put_button("Open a file", phosphor::FILE, put(2));
                    Button::new("Close pane", put(3))
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build();
                },
            );
        },
    );
}

/// The modifier of the pane at position `index` as a place a carried tab can be let go over:
/// the pane and the edge of it the pointer is near.
fn pane_place(index: usize) -> Modifier {
    pm_components::events::carried_over(false, move |at, rect| {
        let area = Area {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.size.width,
            height: rect.size.height,
        };
        Over::Pane(index, area.side_at(at))
    })
}

/// Emits the pane `placed`: its strip of tabs over the tab it shows, the button for the terminal in its strip when `toggle`, its views given `extra` logical pixels of height beyond their own, and, while a tab is carried over it, the highlight over the part of it the tab would take.
#[track_caller]
fn pane(state: &WorkspaceState, placed: &Placed, toggle: bool, extra: f32) {
    let group = placed.group;
    rustle::stack_with(
        Modifier::new()
            .fill_max_height()
            .weight(1.0)
            .clip()
            .control(
                Target::Window(WindowEvent::Pane(placed.index)),
                send(Event::Window(WindowEvent::Pane(placed.index))),
            )
            .chain(pm_components::bounds::placed(
                Target::Window(WindowEvent::Pane(placed.index)),
                Modifier::new(),
            ))
            .chain(pane_place(placed.index)),
        || {
            column_arranged(
                Modifier::new().fill_max_size(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    strip(state, group, toggle);
                    match group.items.get(group.active) {
                        Some(item) => {
                            item_view(state, item, placed.area, placed.area.height + extra)
                        }
                        None => empty_pane(state, placed.index),
                    }
                },
            );
            if let Some((_, side)) = state
                .pane_drag
                .as_ref()
                .and_then(|drag| drag.over)
                .filter(|(index, _)| *index == placed.index)
            {
                drop_zone(side);
            }
        },
    );
}

/// Emits the highlight over the part of a pane's box a tab let go of on `side` would take: the whole box for its middle, the half by an edge for an edge, inset a little and rounded.
#[track_caller]
fn drop_zone(side: Side) {
    let zone = match side {
        Side::Center => Modifier::new().fill_max_size(),
        Side::Left => Modifier::new()
            .fill_max_width_with(0.5)
            .fill_max_height()
            .align(BoxAlignment::CenterStart),
        Side::Right => Modifier::new()
            .fill_max_width_with(0.5)
            .fill_max_height()
            .align(BoxAlignment::CenterEnd),
        Side::Top => Modifier::new()
            .fill_max_width()
            .fill_max_height_with(0.5)
            .align(BoxAlignment::TopCenter),
        Side::Bottom => Modifier::new()
            .fill_max_width()
            .fill_max_height_with(0.5)
            .align(BoxAlignment::BottomCenter),
    };
    rustle::spacer(
        zone.z_index(50)
            .padding(4.0)
            .alpha(0.18)
            .background_rounded(theme::color(Role::Accent), Radius::Lg.px()),
    );
}

/// How a split lays out what [`laid`] emits in it, as a Compose custom `Layout` measures:
/// the first side `ratio` of the split's length along `axis`, rounded to a whole pixel,
/// then the line between the sides one pixel thick, the handle on that line taking no
/// room, and the second side the rest; each as long across as the split.
struct Split {
    /// Which way the sides lie.
    axis: Axis,
    /// The share of the split's length the first side takes.
    ratio: f32,
}

impl Split {
    /// The size `along` long on the split's axis and `across` the other way.
    fn size(&self, along: f32, across: f32) -> rustle::Size {
        match self.axis {
            Axis::Row => rustle::Size::new(along, across),
            Axis::Column => rustle::Size::new(across, along),
        }
    }

    /// The place `along` down the split's axis from its start.
    fn offset(&self, along: f32) -> rustle::Offset {
        match self.axis {
            Axis::Row => rustle::Offset::new(along, 0.0),
            Axis::Column => rustle::Offset::new(0.0, along),
        }
    }
}

impl rustle::Layout for Split {
    /// Measures the first side, the line, the handle and the second side, in that order,
    /// each exactly at its length, and places them one after another along the axis.
    fn measure(
        &self,
        children: &mut rustle::Children,
        constraints: rustle::Constraints,
    ) -> rustle::LayoutResult {
        let (extent, across) = match self.axis {
            Axis::Row => (constraints.max_width, constraints.max_height),
            Axis::Column => (constraints.max_height, constraints.max_width),
        };
        let first = (extent * self.ratio).round();
        let lengths = [first, 1.0, 0.0, (extent - first - 1.0).max(0.0)];
        let starts = [0.0, first, first, first + 1.0];
        let mut positions = Vec::with_capacity(children.len());
        for index in 0..children.len() {
            let length = lengths.get(index).copied().unwrap_or(0.0);
            let size = self.size(length, across);
            children.measure(index, rustle::Constraints::tight(size));
            positions.push(self.offset(starts.get(index).copied().unwrap_or(0.0)));
        }
        rustle::LayoutResult {
            size: self.size(extent, across),
            positions,
        }
    }
}

/// Emits the panes of `layout`, the one counted next being `placed[*next]` and the handle of the split counted next being the `*seam`-th, their views given `extra` logical pixels of height beyond their own; the pane at the top right corner of the panes, which `corner` says this part reaches, holds the button for the terminal. A split lays its sides out by [`Split`], with the handle that resizes it lying on the line between them.
fn laid(
    state: &WorkspaceState,
    layout: &Layout,
    placed: &[Placed],
    (next, seam): (&mut usize, &mut usize),
    (corner, extra): (bool, f32),
) {
    match layout {
        Layout::Group(_) => {
            let Some(here) = placed.get(*next) else {
                return;
            };
            *next += 1;
            rustle::key(here.index, || {
                region(
                    (here.index, here.area, corner, extra, state.inner_sizes()),
                    || {
                        pane(state, here, corner, extra);
                    },
                );
            });
        }
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let id = Target::Window(WindowEvent::SplitSash(*seam));
            *seam += 1;
            let (first_corner, second_corner) = match axis {
                Axis::Row => (false, corner),
                Axis::Column => (corner, false),
            };
            let side = || Modifier::new().fill_max_size();
            rustle::layout(
                Modifier::new().fill_max_height().weight(1.0).clip(),
                Split {
                    axis: *axis,
                    ratio: ratio.get(),
                },
                || {
                    row_arranged(side(), Arrangement::Start, Alignment::Start, || {
                        laid(state, first, placed, (next, seam), (first_corner, extra));
                    });
                    match axis {
                        Axis::Row => rustle::vertical_divider(
                            Modifier::new(),
                            1.0,
                            theme::color(Role::Border),
                        ),
                        Axis::Column => rustle::horizontal_divider(
                            Modifier::new(),
                            1.0,
                            theme::color(Role::Border),
                        ),
                    }
                    rustle::key(id, || match axis {
                        Axis::Row => Sash::column(id).between().build(),
                        Axis::Column => Sash::row(id).between().build(),
                    });
                    row_arranged(side(), Arrangement::Start, Alignment::Start, || {
                        laid(state, second, placed, (next, seam), (second_corner, extra));
                    });
                },
            );
        }
    }
}

/// Emits the tab being dragged under the pointer, as a box placed in the window's layer.
fn drag_marks(state: &WorkspaceState) {
    let Some(drag) = state.pane_drag.as_ref() else {
        return;
    };
    if has_tab(state, &drag.item) {
        rustle::stack_with(
            Modifier::new()
                .offset(drag.at.0 + 8.0, drag.at.1 + 8.0)
                .z_index_unclipped(60)
                .shadow_layers(&theme::MEDIUM_SHADOW, Radius::Md.px())
                .border(1.0, theme::color(Role::Border), Radius::Md.px())
                .padding(1.0),
            || item_tab(state, &drag.item, true),
        );
    }
}

/// Emits the main area of a page in a window `window` logical pixels wide and tall: its panes over the terminal dock while the dock shows, with the handle that resizes the dock lying on its top edge.
pub(crate) fn main_area(state: &WorkspaceState, window: (f32, f32)) {
    let area = state.pane_area(window);
    let panes = state.panes();
    let placed = panes.layout.placed(area);
    let page = window.1 - TITLE_BAR_HEIGHT;
    let extra = page - area.height;
    column_arranged(
        Modifier::new().weight(1.0).fill_max_height(),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    let (mut next, mut seam) = (0, 0);
                    laid(
                        state,
                        &panes.layout,
                        &placed,
                        (&mut next, &mut seam),
                        (true, extra),
                    );
                },
            );
            if let Some(session) = state.docked().filter(|_| state.docking()) {
                Sash::row(Target::Window(WindowEvent::TerminalSash))
                    .between()
                    .build();
                region((area, page, state.inner_sizes()), || {
                    terminal::terminal(state, session, area.width, page);
                });
            }
        },
    );
}

/// Emits, to lay over the whole window from its layer, the tab being dragged between panes.
pub(crate) fn main_over(state: &WorkspaceState) {
    drag_marks(state);
}

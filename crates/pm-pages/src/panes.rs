//! The main area of a page cut into panes: each pane's strip of tabs above the tab it shows, the handles between panes, and, while a tab is dragged, where it would land.

use pm_components::areas::WindowEvent;
use pm_components::areas::{
    BrowserEvent, ChatEvent, DialogEvent, DiffEvent, EditorEvent, SearchEvent,
};
use pm_components::theme::Indicated;
use pm_components::theme::ShadowLayers;

use pm_components::events::Interactive;
use pm_components::events::{Event, send};
use pm_components::target::Target;
use pm_components::theme;
use pm_components::theme::sp;
use pm_components::theme::{Radius, Role, Side};
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{BoxAlignment, Canvas, CursorIcon, Interaction, Modifier};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use pm_components::Menu;
use pm_components::Size;
use pm_components::browser;
use pm_components::button::Variant;
use pm_components::excerpts::{self, Source};
use pm_components::file_icon::file_icon;
use pm_components::open_files::Viewer;
use pm_components::panes::{Area, Axis, Group, Item, Layout, Placed};
use pm_components::parts::{glyph, line, spacer};
use pm_components::revision::region;
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

/// Fills the area of `canvas` with `color`, rounded at its top corners only.
fn fill_top_rounded(canvas: &mut Canvas, color: rustle::Color) {
    let (width, height) = (canvas.width(), canvas.height());
    let corner = TAB_CORNER.min(width.min(height) / 2.0);
    canvas.rounded_rect(0.0, 0.0, width, height, corner, color);
    canvas.rect(0.0, height / 2.0, width, height / 2.0, color);
}

/// The box of a tab answering to `id`: rounded at the top, filled while it is the one its pane shows and while the pointer is on it otherwise, with its padding; as wide as what it holds, running past the end of a strip too narrow for it. A secondary press runs `on_context` when the tab has a menu.
fn tab_modifier(shown: bool, id: Target, on_context: Option<Rc<dyn Fn()>>) -> Modifier {
    let surface = pm_components::theme::color(Role::Surface);
    let hover = pm_components::theme::color(Role::SurfaceHover);
    Modifier::new()
        .wrap_content_width(BoxAlignment::CenterStart, true)
        .height(theme::sp(10))
        .draw_behind_interactive(move |canvas: &mut Canvas, interaction: Interaction| {
            if shown {
                fill_top_rounded(canvas, surface);
            } else if interaction.hovered {
                fill_top_rounded(canvas, hover);
            }
        })
        .cursor(CursorIcon::Pointer)
        .chain(match on_context {
            Some(on_context) => Modifier::new().control_menu(
                id,
                {
                    let pressed = send(Event::pressing(id));
                    move |_| pressed()
                },
                move || on_context(),
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
#[track_caller]
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
            Some(Rc::new(move || operations.open_tab_menu(index)))
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
#[track_caller]
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
#[track_caller]
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

/// Emits a square button in the strip of tabs holding only `icon`, answering to `id`, filled while `on`, saying `hint` when the pointer rests on it and running `on_click` when clicked.
#[track_caller]
fn strip_button(icon: Icon, id: Target, on: bool, hint: &str, on_click: impl Fn() + 'static) {
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
            || glyph(icon, 3.5, tone),
        );
    });
}

/// Emits the buttons at the end of the strip of tabs: the one that opens the menu of panes.
#[track_caller]
fn strip_buttons(state: &WorkspaceState) {
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(1)),
        Alignment::End,
        || {
            strip_button(
                phosphor::SQUARE_SPLIT_HORIZONTAL,
                Target::Window(WindowEvent::PaneMenu),
                state.pane_menu.is_some(),
                "Split pane",
                {
                    let operations = pm_components::events::operations();
                    move || operations.open_pane_menu()
                },
            );
        },
    );
}

/// Emits the menu of panes, opened with its corner at `at`: the pane being worked in split with an empty pane on each side.
#[track_caller]
pub(crate) fn pane_menu(at: (f32, f32)) {
    Menu::new(at.0, at.1)
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
        .build();
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
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
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
#[track_caller]
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
            browser::browser_view(state, width, area.height - pm_components::panes::TABS);
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
#[track_caller]
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
#[track_caller]
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

/// Emits the pane `placed`: its strip of tabs over the tab it shows, the button for the terminal in its strip when `toggle`, its views given `extra` logical pixels of height beyond its own.
#[track_caller]
fn pane(state: &WorkspaceState, placed: &Placed, toggle: bool, extra: f32) {
    let group = placed.group;
    column_arranged(
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
            )),
        Arrangement::Start,
        Alignment::Start,
        || {
            strip(state, group, toggle);
            match group.items.get(group.active) {
                Some(item) => item_view(state, item, placed.area, placed.area.height + extra),
                None => empty_pane(state, placed.index),
            }
        },
    );
}

/// Emits the panes of `layout`, laid out in `area` of the panes' whole area `whole`, the one counted next being `placed[*next]`, their views given `extra` logical pixels of height beyond their own.
fn laid(
    state: &WorkspaceState,
    layout: &Layout,
    placed: &[Placed],
    next: &mut usize,
    (area, whole, extra): (Area, Area, f32),
) {
    match layout {
        Layout::Group(_) => {
            let Some(here) = placed.get(*next) else {
                return;
            };
            *next += 1;
            let toggle = here.area.y <= whole.y + 0.5
                && here.area.x + here.area.width >= whole.x + whole.width - 0.5;
            rustle::key(here.index, || {
                region(
                    (here.index, here.area, toggle, extra, state.inner_sizes()),
                    || {
                        pane(state, here, toggle, extra);
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
            let (one_area, two_area) = area.halves(*axis, *ratio);
            let (sized, edge) = match axis {
                Axis::Row => (
                    Modifier::new().fill_max_height().width(one_area.width),
                    Side::Left,
                ),
                Axis::Column => (
                    Modifier::new().fill_max_width().height(one_area.height),
                    Side::Top,
                ),
            };
            let halves = || {
                row_arranged(sized, Arrangement::Start, Alignment::Start, || {
                    laid(state, first, placed, next, (one_area, whole, extra));
                });
                let across = match axis {
                    Axis::Row => Modifier::new().fill_max_height(),
                    Axis::Column => Modifier::new().fill_max_width(),
                };
                let (left, top, right, bottom) = edge.padding(1.0);
                row_arranged(
                    across
                        .weight(1.0)
                        .draw_behind(theme::edge(edge, 1.0, theme::color(Role::Border)))
                        .padding_each(left, top, right, bottom),
                    Arrangement::Start,
                    Alignment::Start,
                    || laid(state, second, placed, next, (two_area, whole, extra)),
                );
            };
            let outer = Modifier::new().fill_max_height().weight(1.0).clip();
            match axis {
                Axis::Row => row_arranged(outer, Arrangement::Start, Alignment::Start, halves),
                Axis::Column => {
                    column_arranged(outer, Arrangement::Start, Alignment::Start, halves)
                }
            }
        }
    }
}

/// Emits the highlight over the part of a pane a dragged tab would take, and the tab itself under the pointer, as boxes placed in the window's layer.
#[track_caller]
fn drag_marks(state: &WorkspaceState, placed: &[Placed]) {
    let Some(drag) = state.pane_drag.as_ref() else {
        return;
    };
    if let Some((index, side)) = drag.over
        && let Some(here) = placed.get(index)
    {
        let zone = here.area.of_side(side);
        rustle::spacer(
            Modifier::new()
                .offset(zone.x + 4.0, zone.y + 4.0)
                .z_index_unclipped(50)
                .width((zone.width - 8.0).max(0.0))
                .height((zone.height - 8.0).max(0.0))
                .alpha(0.18)
                .background_rounded(theme::color(Role::Accent), Radius::Lg.px()),
        );
    }
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

/// Emits the main area of a page in a window `window` logical pixels wide and tall: its panes over the terminal dock while the dock shows.
#[track_caller]
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
                    let mut next = 0;
                    laid(
                        state,
                        &panes.layout,
                        &placed,
                        &mut next,
                        (area, area, extra),
                    );
                },
            );
            if let Some(session) = state.docked().filter(|_| state.docking()) {
                region((area, page, state.inner_sizes()), || {
                    terminal::terminal(state, session, area.width, page);
                });
            }
        },
    );
}

/// Emits, to lay over the whole window from its layer, the handles between the panes of the main area of a window `window` logical pixels wide and tall, and the marks of a tab being dragged.
#[track_caller]
pub(crate) fn main_over(state: &WorkspaceState, window: (f32, f32)) {
    let area = state.pane_area(window);
    let panes = state.panes();
    let placed = panes.layout.placed(area);
    for seam in panes.layout.seams(area) {
        rustle::key(seam.id, || match seam.axis {
            Axis::Row => Sash::column(seam.id).at(seam.at, seam.length).build(),
            Axis::Column => Sash::row(seam.id).at(seam.at, seam.length).build(),
        });
    }
    drag_marks(state, &placed);
}

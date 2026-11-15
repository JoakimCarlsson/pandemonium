//! The browser pane: the strip of tabs, the bar with the address and the page's buttons, the page with the developer tools beside it, the find bar and the menu over the page, and the question before clearing site data.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    BrowserShortcut, BrowserView, BrowserViewVariant, ConfirmDialog, IconButton, Label,
    LabelVariant, Menu, Sash, Size, Tab, TextInput,
};
use pm_shell::{
    Browser, BrowserAction, BrowserTab, PageCapture, PageCommand, PageSearch, Surface, TOOLS_LEAST,
    ZoomStep,
};

use crate::actions::Actions;
use crate::browser_safety::{certificate_page, download_bar, question_bar};

/// How tall the strip of tabs is, in logical pixels.
const STRIP_HEIGHT: f32 = 34.0;

/// How tall the bar with the address is, in logical pixels.
const BAR_HEIGHT: f32 = 40.0;

/// How wide a tab of the strip is when there is room, in logical pixels.
const TAB_WIDTH: f32 = 180.0;

/// How wide the line the text to find is typed in, in logical pixels.
const FIND_WIDTH: f32 = 200.0;

/// How wide the menu over the page is, in logical pixels.
const MENU_WIDTH: f32 = 240.0;

/// How wide the page stays at least beside the developer tools, in logical pixels.
const PAGE_LEAST: f32 = 200.0;

/// How far apart the tabs of the strip are, in logical pixels.
const TAB_GAP: f32 = 2.0;

/// The rows of the menu of the camera button: what each takes and what it says.
const CAPTURES: [(PageCapture, &str); 4] = [
    (PageCapture::Screenshot, "Screenshot"),
    (PageCapture::FullPage, "Full page"),
    (PageCapture::Area, "Area"),
    (PageCapture::Console, "Console"),
];

/// What a callback of the pane does with an action of the browser.
type Act = Rc<dyn Fn(BrowserAction)>;

/// A callback that hands `action` to `act` each time it is called.
fn send(act: &Act, action: BrowserAction) -> impl Fn() + 'static {
    let act = act.clone();
    move || act(action.clone())
}

/// Emits the browser pane for `browser`, filling the room it is given.
pub fn browser_pane(browser: &Browser, actions: &Actions) {
    let scheme = color_scheme();
    let act: Act = actions.browser.clone();
    let browser = browser.clone();
    column_with(
        Modifier::new()
            .fill_max_size()
            .background(scheme.background),
        move || {
            tab_strip(&browser, &act);
            address_bar(&browser, &act);
            if let Some(download) = &browser.shown().safety.download {
                download_bar(download, &act);
            }
            let content = browser.clone();
            let acting = act.clone();
            with_constraints(
                Modifier::new().fill_max_width().weight(1.0),
                move |constraints| {
                    let most = (constraints.max_width - PAGE_LEAST).max(TOOLS_LEAST);
                    row_with(Modifier::new().fill_max_size(), || {
                        page_area(&content, &acting);
                        if content.tools {
                            tools_area(&content, most, &acting);
                        }
                    });
                },
            );
            if browser.clearing {
                ConfirmDialog::new(
                    "Clear cookies and site data?",
                    "This signs you out of every site in the browser and removes what sites stored: cookies, local storage, IndexedDB and cached files. The page loads again afterwards.",
                    "Clear",
                )
                .on_confirm(send(&act, BrowserAction::ClearConfirm))
                .on_cancel(send(&act, BrowserAction::ClearCancel))
                .show();
            }
        },
    );
}

/// The strip of tabs: a tab for each, which shows it or closes it, and the button that opens a new one.
fn tab_strip(browser: &Browser, act: &Act) {
    let scheme = color_scheme();
    let scroll = remember_scroll_state();
    reveal_active(&scroll, browser.active);
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(STRIP_HEIGHT)
            .background(scheme.surface_container_low)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(6.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(4.0)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new().weight(1.0).horizontal_scroll(&scroll),
                Arrangement::SpacedBy(pm_components::spacing(TAB_GAP)),
                Alignment::Center,
                || {
                    for (index, tab) in browser.tabs.iter().enumerate() {
                        key(tab.id, || {
                            tab_chip(tab, index, index == browser.active, act)
                        });
                    }
                },
            );
            IconButton::new(phosphor::PLUS)
                .size(Size::Xs)
                .tooltip("New tab")
                .on_click(send(act, BrowserAction::NewTab))
                .show();
        },
    );
}

/// Scrolls the strip `scroll` so the tab at `active` shows, when the tab shown changed.
fn reveal_active(scroll: &ScrollState, active: usize) {
    let shown = remember(|| None::<usize>);
    if shown.get() == Some(active) {
        return;
    }
    shown.set(Some(active));
    let scroll = scroll.clone();
    side_effect(move || {
        let start = active as f32 * (TAB_WIDTH + TAB_GAP);
        let end = start + TAB_WIDTH;
        let (offset, room) = (scroll.offset(), scroll.viewport_length());
        if start < offset {
            scroll.scroll_to(start);
        } else if room > 0.0 && end > offset + room {
            scroll.scroll_to(end - room);
        }
    });
}

/// What the strip says of `tab` while the pointer rests on it: its title and address, and whether an agent opened it or it is locked.
fn tab_tooltip(tab: &BrowserTab) -> String {
    let mut text = tab.label();
    if tab.address != text && !tab.address.is_empty() {
        text = format!("{text}\n{}", tab.address);
    }
    if tab.agent {
        text.push_str("\nOpened by an agent");
    } else if tab.locked {
        text.push_str("\nLocked against agents");
    }
    text
}

/// The tab of `tab` at `index` in the strip, `selected` while it is shown.
fn tab_chip(tab: &BrowserTab, index: usize, selected: bool, act: &Act) {
    let icon = if tab.locked {
        phosphor::LOCK_SIMPLE
    } else if tab.agent {
        phosphor::ROBOT
    } else {
        phosphor::GLOBE
    };
    let label: String = tab.label().chars().take(28).collect();
    let tint = tab.agent.then(|| color_scheme().tertiary);
    let hint = tab_tooltip(tab);
    pm_components::tip(&hint, || {
        column_with(Modifier::new().width_in(0.0, TAB_WIDTH), || {
            Tab::new(label)
                .icon(icon)
                .icon_tint(tint)
                .size(Size::Xs)
                .selected(selected)
                .on_click(send(act, BrowserAction::SelectTab(index)))
                .on_close(send(act, BrowserAction::CloseTab(index)))
                .show();
        });
    });
}

/// The bar above the page: back, forward and reload, the address, the zoom while it is not 100%, the lock, the find, the developer tools, the menu and closing the browser.
fn address_bar(browser: &Browser, act: &Act) {
    let scheme = color_scheme();
    let tab = browser.shown().clone();
    let tools = browser.tools;
    let act = act.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(BAR_HEIGHT)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(6.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(2.0)),
        Alignment::Center,
        move || {
            for (glyph, hint, action) in [
                (phosphor::ARROW_LEFT, "Back", BrowserAction::Back),
                (phosphor::ARROW_RIGHT, "Forward", BrowserAction::Forward),
                (
                    phosphor::ARROW_CLOCKWISE,
                    "Reload (Ctrl+R)",
                    BrowserAction::Reload,
                ),
            ] {
                IconButton::new(glyph)
                    .size(Size::Sm)
                    .tooltip(hint)
                    .on_click(send(&act, action))
                    .show();
            }
            spacer(Modifier::new().width(pm_components::spacing(4.0)));
            let typed = act.clone();
            let go = act.clone();
            key(tab.id, || {
                TextInput::new(tab.address.clone())
                    .placeholder("Search or type an address")
                    .size(Size::Sm)
                    .autofocus()
                    .select_on_focus()
                    .modifier(Modifier::new().weight(1.0))
                    .on_change(move |text| typed(BrowserAction::Address(text)))
                    .on_submit(move |_| go(BrowserAction::Go))
                    .show();
            });
            spacer(Modifier::new().width(pm_components::spacing(4.0)));
            if (tab.zoom - 1.0).abs() > 0.001 {
                pm_components::Button::new(format!("{:.0}%", tab.zoom * 100.0))
                    .size(Size::Xs)
                    .tooltip("Reset zoom (Ctrl+0)")
                    .on_click(send(&act, BrowserAction::Zoom(ZoomStep::Reset)))
                    .show();
            }
            let lock = if tab.locked {
                phosphor::LOCK_SIMPLE
            } else {
                phosphor::LOCK_SIMPLE_OPEN
            };
            IconButton::new(lock)
                .size(Size::Sm)
                .active(tab.locked)
                .disabled(tab.agent)
                .tooltip(lock_tooltip(&tab))
                .on_click(send(&act, BrowserAction::Lock))
                .show();
            IconButton::new(phosphor::CURSOR_CLICK)
                .size(Size::Sm)
                .active(tab.picking)
                .tooltip("Pick an element for the prompt")
                .on_click(send(&act, BrowserAction::Pick))
                .show();
            IconButton::new(phosphor::CAMERA)
                .size(Size::Sm)
                .active(tab.area)
                .tooltip("Capture for the prompt")
                .on_click(send(&act, BrowserAction::CaptureMenu))
                .show();
            IconButton::new(phosphor::MAGNIFYING_GLASS)
                .size(Size::Sm)
                .active(tab.search.is_some())
                .tooltip("Find in page (Ctrl+F)")
                .on_click(send(&act, BrowserAction::Find))
                .show();
            IconButton::new(phosphor::CODE)
                .size(Size::Sm)
                .active(tools)
                .tooltip("Developer tools")
                .on_click(send(&act, BrowserAction::ToggleTools))
                .show();
            IconButton::new(phosphor::DOTS_THREE_VERTICAL)
                .size(Size::Sm)
                .tooltip("More")
                .on_click(send(&act, BrowserAction::Menu(Some((-1.0, 0.0)))))
                .show();
            IconButton::new(phosphor::X)
                .size(Size::Sm)
                .tooltip("Close the browser")
                .on_click(send(&act, BrowserAction::Close))
                .show();
        },
    );
}

/// What the lock button of the bar says of `tab` while the pointer rests on it.
fn lock_tooltip(tab: &BrowserTab) -> &'static str {
    if tab.agent {
        "An agent opened this tab, so it cannot be locked"
    } else if tab.locked {
        "Locked against agents; click to unlock"
    } else {
        "Lock this tab against agents"
    }
}

/// The page of the tab shown, or what stands in for it, with the question, the find bar and the menu laid over it.
fn page_area(browser: &Browser, act: &Act) {
    let tab = browser.shown().clone();
    let menu = browser.menu;
    let act = act.clone();
    let mut modifier = Modifier::new().weight(1.0).fill_max_height();
    if tab.area {
        let cancel = act.clone();
        modifier = modifier.on_preview_key_event(move |event| {
            let escape = event.pressed && event.key == rustle::Key::Escape;
            if escape {
                cancel(BrowserAction::CancelArea);
            }
            escape
        });
    }
    stack_with(modifier, move || {
        if let Some(certificate) = &tab.safety.certificate {
            certificate_page(certificate, &act);
        } else {
            page_view(&tab, &act);
        }
        if let Some(question) = tab.safety.questions.first() {
            column_with(
                Modifier::new()
                    .align(BoxAlignment::TopStart)
                    .fill_max_width(),
                || question_bar(question, tab.safety.questions.len() - 1, &tab.safety, &act),
            );
        }
        if let Some(search) = &tab.search {
            find_bar(search, &act);
        }
        if tab.area {
            area_bar(&act);
        }
        if let Some(menu) = menu {
            let act = act.clone();
            with_constraints(Modifier::new().fill_max_size(), move |constraints| {
                let room = (constraints.max_width, constraints.max_height);
                if menu.capture {
                    capture_menu(room.0, &act);
                } else {
                    page_menu(fitted(menu.at, room), room.0, &act);
                }
            });
        }
    });
}

/// The window of the page of `tab`, or why it cannot be shown.
fn page_view(tab: &BrowserTab, act: &Act) {
    let input = act.clone();
    let resize = act.clone();
    let focus = act.clone();
    let shortcut = act.clone();
    let menu = act.clone();
    let placeholder = tab.notice.clone().unwrap_or_else(|| {
        if tab.page.is_none() {
            format!("Loading {}", tab.address)
        } else {
            String::new()
        }
    });
    let frame = if tab.notice.is_some() {
        None
    } else {
        tab.page.as_ref().map(|picture| picture.0.clone())
    };
    BrowserView::new(frame)
        .cursor(tab.pointer)
        .placeholder(placeholder)
        .modifier(Modifier::new().fill_max_size())
        .on_input(move |event| input(BrowserAction::Input(Surface::Page, event)))
        .on_resize(move |width, height, scale| {
            resize(BrowserAction::Resized(Surface::Page, width, height, scale))
        })
        .on_focus(move |focused| focus(BrowserAction::Focused(Surface::Page, focused)))
        .on_shortcut(move |key| shortcut(shortcut_action(key)))
        .on_menu(move |x, y| menu(BrowserAction::Menu(Some((x, y)))))
        .show();
}

/// What the browser key `shortcut` asks of the browser.
fn shortcut_action(shortcut: BrowserShortcut) -> BrowserAction {
    match shortcut {
        BrowserShortcut::Find => BrowserAction::Find,
        BrowserShortcut::FindNext => BrowserAction::FindStep { back: false },
        BrowserShortcut::FindPrevious => BrowserAction::FindStep { back: true },
        BrowserShortcut::ZoomIn => BrowserAction::Zoom(ZoomStep::In),
        BrowserShortcut::ZoomOut => BrowserAction::Zoom(ZoomStep::Out),
        BrowserShortcut::ZoomReset => BrowserAction::Zoom(ZoomStep::Reset),
        BrowserShortcut::Reload => BrowserAction::Reload,
        BrowserShortcut::HardReload => BrowserAction::HardReload,
        BrowserShortcut::NewTab => BrowserAction::NewTab,
        BrowserShortcut::CloseTab => BrowserAction::CloseShownTab,
        BrowserShortcut::Back => BrowserAction::Back,
        BrowserShortcut::Forward => BrowserAction::Forward,
    }
}

/// The sash and the developer tools beside the page, at most `most` wide so the page keeps room.
fn tools_area(browser: &Browser, most: f32, act: &Act) {
    let scheme = color_scheme();
    let width = browser.tools_width.min(most);
    let frame = browser.tools_page.as_ref().map(|picture| picture.0.clone());
    let resize = act.clone();
    key("browser-tools-sash", || {
        Sash::new(width)
            .range(TOOLS_LEAST, most)
            .after()
            .on_resize(move |width| resize(BrowserAction::ToolsWidth(width)))
            .show();
    });
    let input = act.clone();
    let sized = act.clone();
    let focus = act.clone();
    column_with(
        Modifier::new()
            .width(width)
            .fill_max_height()
            .background(scheme.background),
        move || {
            BrowserView::new(frame)
                .variant(BrowserViewVariant::Tools)
                .placeholder("Opening the developer tools")
                .modifier(Modifier::new().fill_max_size())
                .on_input(move |event| input(BrowserAction::Input(Surface::Tools, event)))
                .on_resize(move |width, height, scale| {
                    sized(BrowserAction::Resized(Surface::Tools, width, height, scale))
                })
                .on_focus(move |focused| focus(BrowserAction::Focused(Surface::Tools, focused)))
                .show();
        },
    );
}

/// The bar for finding in the page, over its top right corner: the text, the count of matches, the steps and closing it.
fn find_bar(search: &PageSearch, act: &Act) {
    let scheme = color_scheme();
    let (tally, failed) = search.tally();
    let query = search.query.clone();
    let act = act.clone();
    row_arranged(
        Modifier::new()
            .align(BoxAlignment::TopEnd)
            .offset(-16.0, 8.0)
            .background(scheme.surface_container)
            .border(1.0, scheme.outline_variant, 8.0)
            .padding_xy(6.0, 4.0),
        Arrangement::SpacedBy(pm_components::spacing(4.0)),
        Alignment::Center,
        move || {
            let typed = act.clone();
            let step = act.clone();
            let back = act.clone();
            let close = act.clone();
            TextInput::new(query.clone())
                .placeholder("Find in page")
                .size(Size::Xs)
                .autofocus()
                .select_on_focus()
                .modifier(Modifier::new().width(FIND_WIDTH))
                .on_change(move |text| typed(BrowserAction::FindTyped(text)))
                .on_submit(move |_| step(BrowserAction::FindStep { back: false }))
                .on_shift_submit(move |_| back(BrowserAction::FindStep { back: true }))
                .on_escape(move || close(BrowserAction::FindClose))
                .show();
            let label = Label::new(tally.clone())
                .size(Size::Xs)
                .modifier(Modifier::new().width(72.0));
            if failed {
                label.colour(scheme.error).show();
            } else {
                label.variant(LabelVariant::Muted).show();
            }
            IconButton::new(phosphor::CARET_UP)
                .size(Size::Xs)
                .tooltip("Previous match (Shift+Enter)")
                .on_click(send(&act, BrowserAction::FindStep { back: true }))
                .show();
            IconButton::new(phosphor::CARET_DOWN)
                .size(Size::Xs)
                .tooltip("Next match (Enter)")
                .on_click(send(&act, BrowserAction::FindStep { back: false }))
                .show();
            IconButton::new(phosphor::X)
                .size(Size::Xs)
                .tooltip("Close (Escape)")
                .on_click(send(&act, BrowserAction::FindClose))
                .show();
        },
    );
}

/// How tall the menu over the page is, in logical pixels: its rows, its separators and its padding.
fn menu_height() -> f32 {
    let separators = PageCommand::ROWS
        .iter()
        .filter(|(_, separated)| *separated)
        .count();
    PageCommand::ROWS.len() as f32 * 22.0 + separators as f32 * 9.0 + 12.0
}

/// The point `at` moved up and left so the menu opened there ends inside a page as big as `room`, as far as it fits; a point left of the page, which means below the menu button, stays so.
fn fitted(at: (f32, f32), room: (f32, f32)) -> (f32, f32) {
    let x = if at.0 < 0.0 {
        at.0
    } else {
        at.0.min(room.0 - MENU_WIDTH).max(0.0)
    };
    (x, at.1.min(room.1 - menu_height()).max(0.0))
}

/// The bar over the top of the page while the reader drags a rectangle over it to capture, with the button that stops.
fn area_bar(act: &Act) {
    let scheme = color_scheme();
    let act = act.clone();
    row_arranged(
        Modifier::new()
            .align(BoxAlignment::TopCenter)
            .offset(0.0, 8.0)
            .background(scheme.surface_container)
            .border(1.0, scheme.outline_variant, 8.0)
            .padding_xy(10.0, 4.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        move || {
            Label::new("Drag over the page to capture an area · Escape cancels")
                .size(Size::Xs)
                .show();
            pm_components::Button::new("Cancel")
                .size(Size::Xs)
                .on_click(send(&act, BrowserAction::CancelArea))
                .show();
        },
    );
}

/// The menu of the camera button, below it at the top right of a page `across` wide: the captures it can add to the prompt.
fn capture_menu(across: f32, act: &Act) {
    let act = act.clone();
    let width = MENU_WIDTH / 2.0 + 40.0;
    stack_with(
        Modifier::new().offset((across - width - MENU_WIDTH / 2.0).max(0.0), 0.0),
        move || {
            let mut menu = Menu::new()
                .width(width)
                .size(Size::Xs)
                .on_dismiss(send(&act, BrowserAction::Menu(None)));
            for (kind, label) in CAPTURES {
                menu = menu.item(label, send(&act, BrowserAction::Capture(kind)));
            }
            menu.show();
        },
    );
}

/// The menu over a page `across` wide, opened at `at`, a point of the page, or below the menu button at the top right when `at` is to the left of the page.
fn page_menu(at: (f32, f32), across: f32, act: &Act) {
    let modifier = if at.0 < 0.0 {
        Modifier::new().offset((across - MENU_WIDTH - 8.0).max(0.0), 0.0)
    } else {
        Modifier::new().offset(at.0, at.1)
    };
    let act = act.clone();
    stack_with(modifier, move || {
        let mut menu = Menu::new()
            .width(MENU_WIDTH)
            .size(Size::Xs)
            .on_dismiss(send(&act, BrowserAction::Menu(None)));
        for (command, separated) in PageCommand::ROWS {
            menu = menu.item(command.label(), send(&act, BrowserAction::Command(command)));
            if let Some(keys) = command.keys() {
                menu = menu.hint(keys);
            }
            if separated {
                menu = menu.separator();
            }
        }
        menu.show();
    });
}

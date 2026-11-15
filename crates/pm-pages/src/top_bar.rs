//! The top bar: history arrows, the command centre field, panel toggles,
//! settings and the window's controls.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{PointerButton, PressEvent};

use pm_components::{Button, ButtonVariant, IconButton, SearchField, Size};
use pm_shell::{Page, Workspace};

use crate::actions::{Actions, Destination, Panel, UpdateAction, WindowAction};

/// Height of the top bar.
pub const TOP_BAR_HEIGHT: f32 = 40.0;

/// Width of the command centre field.
const COMMAND_CENTRE_WIDTH: f32 = 330.0;

/// Emits the top bar for `workspace`.
pub fn top_bar(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    let placeholder = place_name(workspace);
    let window = actions.window.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(TOP_BAR_HEIGHT)
            .on_press(move |press| title_press(&window, press))
            .background(scheme.background)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(10.0, 0.0),
        Arrangement::SpaceBetween,
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Center,
                || {
                    row_arranged(
                        Modifier::new().on_press(keep_press),
                        Arrangement::SpacedBy(pm_components::spacing(4.0)),
                        Alignment::Center,
                        || {
                            trail_button(
                                actions,
                                phosphor::ARROW_LEFT,
                                WindowAction::Back,
                                workspace.trail.can_go_back(),
                            );
                            trail_button(
                                actions,
                                phosphor::ARROW_RIGHT,
                                WindowAction::Forward,
                                workspace.trail.can_go_forward(),
                            );
                        },
                    );
                },
            );
            let open = actions.command_centre.clone();
            SearchField::new(placeholder)
                .size(Size::Sm)
                .modifier(
                    Modifier::new()
                        .width(COMMAND_CENTRE_WIDTH)
                        .on_press(keep_press),
                )
                .on_click(move || open())
                .show();
            row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::End,
                Alignment::Center,
                || {
                    row_arranged(
                        Modifier::new().on_press(keep_press),
                        Arrangement::SpacedBy(pm_components::spacing(4.0)),
                        Alignment::Center,
                        || {
                            if let Some(offer) = &workspace.update {
                                let update = actions.update.clone();
                                Button::new(format!("Update to {}", offer.version))
                                    .icon(phosphor::CLOUD_ARROW_DOWN)
                                    .variant(ButtonVariant::Primary)
                                    .size(Size::Xs)
                                    .on_click(move || update(UpdateAction::Open))
                                    .show();
                                spacer(Modifier::new().width(pm_components::spacing(8.0)));
                            }
                            panel_button(
                                actions,
                                phosphor::SIDEBAR_SIMPLE,
                                Panel::Sidebar,
                                workspace.sidebar_open,
                            );
                            panel_button(
                                actions,
                                phosphor::SQUARE_HALF_BOTTOM,
                                Panel::Dock,
                                workspace.dock_open,
                            );
                            panel_button(
                                actions,
                                phosphor::SIDEBAR,
                                Panel::Right,
                                workspace.right_open,
                            );
                            let navigate = actions.navigate.clone();
                            IconButton::new(phosphor::GEAR_SIX)
                                .size(Size::Sm)
                                .tooltip("Settings")
                                .on_click(move || navigate(Destination::Settings))
                                .show();
                            spacer(Modifier::new().width(pm_components::spacing(12.0)));
                            window_button(actions, phosphor::MINUS, WindowAction::Minimise);
                            window_button(
                                actions,
                                phosphor::ARROWS_OUT_SIMPLE,
                                WindowAction::Maximise,
                            );
                            window_button(actions, phosphor::X, WindowAction::Close);
                        },
                    );
                },
            );
        },
    );
}

/// Handles a press on the top bar's empty space through `window`: a single primary press drags the window, a double press maximises or restores it. One handler serves both, since only the topmost press handler of an area runs.
fn title_press(window: &Rc<dyn Fn(WindowAction)>, press: PressEvent) {
    if press.button != PointerButton::Primary {
        return;
    }
    match press.click_count {
        1 => window(WindowAction::Drag),
        2 => window(WindowAction::Maximise),
        _ => {}
    }
}

/// Takes a press on the controls of a title strip, such as the top bar or the strip of the settings and folder picker windows, so that it does not reach the strip beneath and drag the window, leaving the controls' own clicks to them.
pub(crate) fn keep_press(_press: PressEvent) {}

/// What the command centre field says: the page showing, or the open
/// session and its project.
fn place_name(workspace: &Workspace) -> String {
    match &workspace.page {
        Page::History => "History".to_owned(),
        Page::Extensions => "Extensions".to_owned(),
        Page::Overview(path) => path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .to_owned(),
        Page::Home | Page::Session(_) => match workspace.open {
            Some(_) => format!("{} · {}", open_project(workspace), workspace.title),
            None => "New chat".to_owned(),
        },
    }
}

/// A small button walking the trail for `action`, faint when `enabled` is
/// false and there is nowhere to go.
#[track_caller]
fn trail_button(actions: &Actions, glyph: Icon, action: WindowAction, enabled: bool) {
    let scheme = color_scheme();
    let window = actions.window.clone();
    let label = match action {
        WindowAction::Back => "Go back",
        _ => "Go forward",
    };
    let mut button = IconButton::new(glyph).size(Size::Sm).tooltip(label);
    if !enabled {
        button = button.tint(scheme.on_surface_variant);
    }
    button.on_click(move || window(action)).show();
}

/// The name of the project holding the open session.
fn open_project(workspace: &Workspace) -> String {
    let Some(path) = &workspace.open else {
        return String::new();
    };
    let mut projects = &workspace.projects;
    let mut name = String::new();
    for index in &path[..path.len().saturating_sub(1)] {
        let Some(project) = projects.get(*index) else {
            break;
        };
        name = project.name.clone();
        projects = &project.children;
    }
    name
}

/// A small button asking the window for `action`.
#[track_caller]
fn window_button(actions: &Actions, glyph: Icon, action: WindowAction) {
    let window = actions.window.clone();
    let label = match action {
        WindowAction::Minimise => "Minimise",
        WindowAction::Maximise => "Maximise or restore",
        _ => "Close",
    };
    IconButton::new(glyph)
        .size(Size::Sm)
        .tooltip(label)
        .on_click(move || window(action))
        .show();
}

/// A small button showing or hiding `panel`, shown switched on while `open`.
#[track_caller]
fn panel_button(actions: &Actions, glyph: Icon, panel: Panel, open: bool) {
    let toggle = actions.toggle.clone();
    let label = match panel {
        Panel::Sidebar => "Toggle the sidebar",
        Panel::Dock => "Toggle the bottom panel",
        Panel::Right => "Toggle the right panel",
    };
    IconButton::new(glyph)
        .size(Size::Sm)
        .active(open)
        .tooltip(label)
        .on_click(move || toggle(panel))
        .show();
}

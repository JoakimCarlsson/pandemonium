//! The folder picker, drawn in a window of its own: the machines on the
//! left, the path above as a trail with the steps back, forward and up, the
//! folders of the one shown as a filtered list, and the button that takes
//! the folder shown. It looks at this machine or another one alike.

use std::rc::Rc;

use pm_components::{
    AppTheme, Button, ButtonVariant, Divider, IconButton, Label, LabelVariant, Leading, Notice,
    NoticeVariant, Size, TextInput, Tile, app_theme,
};
use pm_shell::{FolderPicker, PickerAction, ProjectAction, Reach};
use rustle::prelude::*;
use rustle::{Key, KeyEvent};

use crate::top_bar::keep_press;

/// The folder picker window's first size, in logical pixels.
pub const PICKER_SIZE: (f32, f32) = (720.0, 480.0);

/// How tall the picker's title strip is.
const TITLE_HEIGHT: f32 = 36.0;

/// How wide the list of machines is.
const MACHINES_WIDTH: f32 = 170.0;

/// What the folder picker window asks the shell to do.
#[derive(Clone)]
pub struct PickerActions {
    /// Works the picker, or adds the folder chosen as a project.
    pub act: Rc<dyn Fn(ProjectAction)>,
    /// Starts moving the window with the pointer.
    pub drag: Rc<dyn Fn()>,
}

impl PickerActions {
    /// Actions that do nothing, for snapshots.
    pub fn inert() -> Self {
        Self {
            act: Rc::new(|_| {}),
            drag: Rc::new(|| {}),
        }
    }
}

/// Emits the folder picker in `picker` in `theme`, calling `actions` when the reader acts.
pub fn folder_picker_page(picker: &FolderPicker, theme: AppTheme, actions: &PickerActions) {
    app_theme(theme, || {
        let scheme = color_scheme();
        let active = remember(|| None::<usize>);
        let shown_at = remember(String::new);
        if shown_at.get() != picker.path {
            shown_at.set(picker.path.clone());
            active.set(None);
        }
        let early = picker_keys(picker, actions, &active, true);
        let late = picker_keys(picker, actions, &active, false);
        column_with(
            Modifier::new()
                .fill_max_size()
                .background(scheme.background)
                .on_preview_key_event(early)
                .on_key_event(late),
            || {
                title_strip(actions);
                row_with(Modifier::new().fill_max_width().weight(1.0), || {
                    machines(picker, actions);
                    column_with(Modifier::new().weight(1.0).fill_max_height(), || {
                        trail(picker, actions);
                        folders(picker, actions, active.get());
                        footer(picker, actions);
                    });
                });
            },
        );
    });
}

/// The picker's keys: the up and down arrows, Home and End move the folder marked as `active`, Enter goes into it or adds the folder shown when none is marked, Alt with the arrows goes back, forward and up, Backspace goes up when the filter is empty, and Escape closes the picker. Taken `early`, before the filter sees them, are the arrows, Enter and Escape; the rest only when the filter left them.
fn picker_keys(
    picker: &FolderPicker,
    actions: &PickerActions,
    active: &State<Option<usize>>,
    early: bool,
) -> impl Fn(&KeyEvent) -> bool + 'static {
    let names: Vec<String> = picker.shown().into_iter().map(str::to_owned).collect();
    let filtered = !picker.filter.is_empty();
    let reached = picker.reach == Reach::Reached;
    let act = actions.act.clone();
    let active = active.clone();
    move |event: &KeyEvent| {
        let first = matches!(
            event.key,
            Key::ArrowDown
                | Key::ArrowUp
                | Key::ArrowLeft
                | Key::ArrowRight
                | Key::Enter
                | Key::Escape
        );
        if !event.pressed || first != early {
            return false;
        }
        let modifiers = event.modifiers;
        let picked = |action: PickerAction| {
            act(ProjectAction::Picker(action));
            true
        };
        let last = names.len().saturating_sub(1);
        let step = |to: usize| {
            active.set((!names.is_empty()).then_some(to.min(last)));
            true
        };
        match (&event.key, modifiers.alt) {
            (Key::ArrowLeft, true) => picked(PickerAction::Back),
            (Key::ArrowRight, true) => picked(PickerAction::Forward),
            (Key::ArrowUp, true) => picked(PickerAction::Up),
            (Key::Backspace, false) if !filtered => picked(PickerAction::Up),
            (Key::Escape, _) => picked(PickerAction::Close),
            (Key::ArrowDown, false) => step(active.get().map_or(0, |at| (at + 1).min(last))),
            (Key::ArrowUp, false) => step(active.get().map_or(0, |at| at.saturating_sub(1))),
            (Key::Home, false) => step(0),
            (Key::End, false) => step(last),
            (Key::Enter, false) => match active.get().and_then(|at| names.get(at)) {
                Some(name) => picked(PickerAction::Into(name.clone())),
                None if filtered && names.len() == 1 => {
                    picked(PickerAction::Into(names[0].clone()))
                }
                None if reached => picked(PickerAction::Choose),
                None => false,
            },
            _ => false,
        }
    }
}

/// The callback that does `action` in the picker.
fn doing(actions: &PickerActions, action: PickerAction) -> impl Fn() + 'static {
    let act = actions.act.clone();
    move || act(ProjectAction::Picker(action.clone()))
}

/// The strip across the top: the window's name, moving it when pressed, and the button closing it.
fn title_strip(actions: &PickerActions) {
    let scheme = color_scheme();
    let drag = actions.drag.clone();
    let close = doing(actions, PickerAction::Close);
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(TITLE_HEIGHT)
            .background(scheme.surface_container_low)
            .border_bottom_hairline(scheme.outline_variant)
            .on_press(move |press| {
                if press.click_count == 1 {
                    drag();
                }
            })
            .padding_xy(12.0, 0.0),
        Arrangement::SpaceBetween,
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Center,
                || {
                    icon(phosphor::FOLDER_OPEN, 14.0, scheme.on_surface_variant);
                    Label::new("Choose a folder")
                        .variant(LabelVariant::Muted)
                        .show();
                },
            );
            row_with(Modifier::new().on_press(keep_press), || {
                IconButton::new(phosphor::X)
                    .size(Size::Sm)
                    .tooltip("Close")
                    .on_click(close)
                    .show();
            });
        },
    );
}

/// The machines the picker can look at, the one looked at chosen.
fn machines(picker: &FolderPicker, actions: &PickerActions) {
    let scheme = color_scheme();
    column_with(
        Modifier::new()
            .width(MACHINES_WIDTH)
            .fill_max_height()
            .background(scheme.surface_container_low)
            .border_right_hairline(scheme.outline_variant)
            .padding(8.0),
        || {
            Label::new("Machines")
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding_xy(8.0, 4.0))
                .show();
            for machine in &picker.machines {
                key(machine.name.clone(), || {
                    let glyph = if machine.host.is_some() {
                        phosphor::HARD_DRIVES
                    } else {
                        phosphor::DESKTOP
                    };
                    Tile::new(machine.name.clone())
                        .leading(Leading::Icon(glyph))
                        .selected(machine.host == picker.host)
                        .on_click(doing(actions, PickerAction::Open(machine.host.clone())))
                        .show();
                });
            }
            spacer(Modifier::new().height(pm_components::spacing(12.0)));
            Label::new("Places")
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding_xy(8.0, 4.0))
                .show();
            Tile::new("Home")
                .leading(Leading::Icon(phosphor::HOUSE))
                .on_click(doing(actions, PickerAction::Go("~".to_owned())))
                .show();
            Tile::new("Root")
                .leading(Leading::Icon(phosphor::FOLDER))
                .on_click(doing(actions, PickerAction::Go("/".to_owned())))
                .show();
        },
    );
}

/// The steps back, forward and up, the path as a trail of its folders, and the filter.
fn trail(picker: &FolderPicker, actions: &PickerActions) {
    let scheme = color_scheme();
    let filter = actions.act.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(44.0)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(10.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(4.0)),
        Alignment::Center,
        || {
            IconButton::new(phosphor::ARROW_LEFT)
                .size(Size::Sm)
                .tooltip("Back")
                .active(!picker.back.is_empty())
                .on_click(doing(actions, PickerAction::Back))
                .show();
            IconButton::new(phosphor::ARROW_RIGHT)
                .size(Size::Sm)
                .tooltip("Forward")
                .active(!picker.forward.is_empty())
                .on_click(doing(actions, PickerAction::Forward))
                .show();
            IconButton::new(phosphor::ARROW_UP)
                .size(Size::Sm)
                .tooltip("Up one folder")
                .on_click(doing(actions, PickerAction::Up))
                .show();
            spacer(Modifier::new().width(pm_components::spacing(6.0)));
            row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::SpacedBy(pm_components::spacing(2.0)),
                Alignment::Center,
                || {
                    for (place, (name, path)) in picker.trail().into_iter().enumerate() {
                        key(place, || {
                            if place > 1 {
                                Label::new("/")
                                    .variant(LabelVariant::Subtle)
                                    .size(Size::Sm)
                                    .show();
                            }
                            Button::new(name)
                                .variant(ButtonVariant::Ghost)
                                .size(Size::Sm)
                                .on_click(doing(actions, PickerAction::Go(path)))
                                .show();
                        });
                    }
                },
            );
            TextInput::new(picker.filter.clone())
                .autofocus()
                .placeholder("Filter")
                .size(Size::Sm)
                .modifier(Modifier::new().width(180.0))
                .on_change(move |text| filter(ProjectAction::Picker(PickerAction::Filter(text))))
                .show();
        },
    );
}

/// The folders inside the one shown, the one at `active` marked for the keys, or how the machine stands while they are not there.
fn folders(picker: &FolderPicker, actions: &PickerActions, active: Option<usize>) {
    let scroll = remember_scroll_state();
    column_with(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .vertical_scroll(&scroll)
            .padding(8.0),
        || match &picker.reach {
            Reach::Waiting => {
                Label::new(match &picker.host {
                    Some(host) => format!("Reaching {host}…"),
                    None => "Listing folders…".to_owned(),
                })
                .variant(LabelVariant::Muted)
                .modifier(Modifier::new().padding(12.0))
                .show();
            }
            Reach::Failed(reason) => {
                Notice::new("The folders could not be listed")
                    .detail(reason.clone())
                    .variant(NoticeVariant::Danger)
                    .show();
            }
            Reach::Reached => {
                let shown = picker.shown();
                if shown.is_empty() {
                    Label::new("No folders here")
                        .variant(LabelVariant::Muted)
                        .modifier(Modifier::new().padding(12.0))
                        .show();
                }
                for (place, name) in shown.into_iter().enumerate() {
                    key(name.to_owned(), || {
                        Tile::new(name)
                            .leading(Leading::Icon(phosphor::FOLDER))
                            .selected(active == Some(place))
                            .on_click(doing(actions, PickerAction::Into(name.to_owned())))
                            .show();
                    });
                }
            }
        },
    );
}

/// The address the folder shown is added as, with the buttons that cancel and that add it.
fn footer(picker: &FolderPicker, actions: &PickerActions) {
    Divider::new().show();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(52.0)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            Label::new(picker.address())
                .variant(LabelVariant::Muted)
                .size(Size::Sm)
                .modifier(Modifier::new().weight(1.0))
                .show();
            Button::new("Cancel")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(doing(actions, PickerAction::Close))
                .show();
            Button::new(if picker.for_extension {
                "Install extension"
            } else {
                "Add project"
            })
            .icon(if picker.for_extension {
                phosphor::PLUG
            } else {
                phosphor::FOLDER_PLUS
            })
            .variant(ButtonVariant::Primary)
            .size(Size::Sm)
            .disabled(picker.reach != Reach::Reached)
            .on_click(doing(actions, PickerAction::Choose))
            .show();
        },
    );
}

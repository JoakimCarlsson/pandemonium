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
        column_with(
            Modifier::new()
                .fill_max_size()
                .background(scheme.background),
            || {
                title_strip(actions);
                row_with(Modifier::new().fill_max_width().weight(1.0), || {
                    machines(picker, actions);
                    column_with(Modifier::new().weight(1.0).fill_max_height(), || {
                        trail(picker, actions);
                        folders(picker, actions);
                        footer(picker, actions);
                    });
                });
            },
        );
    });
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
                .placeholder("Filter")
                .size(Size::Sm)
                .modifier(Modifier::new().width(180.0))
                .on_change(move |text| filter(ProjectAction::Picker(PickerAction::Filter(text))))
                .show();
        },
    );
}

/// The folders inside the one shown, or how the machine stands while they are not there.
fn folders(picker: &FolderPicker, actions: &PickerActions) {
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
                for name in shown {
                    key(name.to_owned(), || {
                        Tile::new(name)
                            .leading(Leading::Icon(phosphor::FOLDER))
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
            Button::new("Add project")
                .icon(phosphor::FOLDER_PLUS)
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .disabled(picker.reach != Reach::Reached)
                .on_click(doing(actions, PickerAction::Choose))
                .show();
        },
    );
}

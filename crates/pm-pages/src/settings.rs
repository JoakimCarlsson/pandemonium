//! The settings window: a title strip over the list of sections and the
//! page of the section picked, laid out narrower in a small window.

use std::rc::Rc;

use rustle::prelude::*;

use crate::top_bar::keep_press;

use pm_components::{AppTheme, IconButton, Label, LabelVariant, SectionHeader, Size, app_theme};
use pm_shell::{Section, SettingsModel};

use crate::settings_actions::SettingsActions;
use crate::settings_nav::navigation;

/// The settings window's first size, in logical pixels.
pub const SETTINGS_SIZE: (f32, f32) = (960.0, 720.0);

/// Height of the strip across the top that moves and closes the window.
const TITLE_HEIGHT: f32 = 36.0;

/// How wide the window is at most for its layout to be narrow.
const NARROW_WINDOW: f32 = 760.0;

/// How narrow the page is before rows set their control under their words.
const STACK_BELOW: f32 = 480.0;

/// How the settings window is laid out at its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Layout {
    /// How wide the list of sections is.
    pub(crate) navigation: f32,
    /// The space either side of the page.
    pub(crate) inset: f32,
    /// How wide the page is inside its insets.
    pub(crate) room: f32,
    /// Whether rows set their control under their words.
    pub(crate) stacked: bool,
}

impl Layout {
    /// The layout of a window `width` logical pixels across.
    fn of(width: f32) -> Self {
        let narrow = width <= NARROW_WINDOW;
        let navigation = if narrow { 184.0 } else { 220.0 };
        let inset = if narrow { 20.0 } else { 32.0 };
        let room = (width - navigation - inset * 2.0).max(160.0);
        Self {
            navigation,
            inset,
            room,
            stacked: room < STACK_BELOW,
        }
    }
}

/// Emits the settings window for `model` in `theme`, calling `actions` when the person acts.
pub fn settings_page(model: &SettingsModel, theme: AppTheme, actions: &SettingsActions) {
    let model = Rc::new(model.clone());
    let actions = actions.clone();
    app_theme(theme, || {
        with_constraints(Modifier::new().fill_max_size(), move |constraints| {
            let layout = Layout::of(constraints.max_width);
            let model = model.clone();
            let actions = actions.clone();
            app_theme(theme, move || {
                let scheme = color_scheme();
                column_with(
                    Modifier::new()
                        .fill_max_size()
                        .background(scheme.background),
                    || {
                        title_strip(&actions);
                        row_with(Modifier::new().fill_max_width().weight(1.0), || {
                            navigation(&model, &actions, layout);
                            content(&model, &actions, layout);
                        });
                    },
                );
            });
        });
    });
}

/// The strip across the top: the window's name, moving it when pressed, and the button closing it.
fn title_strip(actions: &SettingsActions) {
    let scheme = color_scheme();
    let drag = actions.drag.clone();
    let close = actions.close.clone();
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
                Arrangement::SpacedBy(8.0),
                Alignment::Center,
                || {
                    icon(phosphor::GEAR_SIX, 14.0, scheme.on_surface_variant);
                    Label::new("Settings").variant(LabelVariant::Muted).show();
                },
            );
            row_with(Modifier::new().on_press(keep_press), || {
                IconButton::new(phosphor::X)
                    .size(Size::Sm)
                    .on_click(move || close())
                    .show();
            });
        },
    );
}

/// The page of the section `model` shows, under its heading, scrolled as a whole.
fn content(model: &Rc<SettingsModel>, actions: &SettingsActions, layout: Layout) {
    let scheme = color_scheme();
    let scroll = remember_scroll_state();
    let section = model.showing();
    key(section, || {
        column_with(
            Modifier::new()
                .weight(1.0)
                .fill_max_height()
                .background(scheme.background)
                .vertical_scroll(&scroll),
            || {
                column_arranged(
                    Modifier::new().fill_max_width().padding_each(
                        layout.inset,
                        24.0,
                        layout.inset,
                        32.0,
                    ),
                    Arrangement::SpacedBy(16.0),
                    Alignment::Start,
                    || {
                        SectionHeader::new(section.title())
                            .trail(section.parent().map(|parent| parent.title().to_owned()))
                            .summary(section.summary())
                            .show();
                        crate::settings_body::body(model, actions, section, layout);
                    },
                );
            },
        );
    });
}

/// The section a snapshot opens on, by its slug, such as `editor-vim`.
pub fn settings_section(slug: &str) -> Option<Section> {
    Section::from_slug(slug)
}

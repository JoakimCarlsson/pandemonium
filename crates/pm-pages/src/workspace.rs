//! The whole window: the top bar over the sidebar, the session view or the
//! welcome page or another page in the middle, the right panel and the dock,
//! with the quick input, the modals and the dialogs over them.

use rustle::prelude::*;

use rustle::KeyEvent;

use pm_components::{
    AppTheme, Button, ButtonVariant, KeyRecorder, Modal, Sash, SashVariant, Size, app_theme,
};
use pm_shell::{Page, PanelSizes, TreeAction, Workspace, name_of};

use crate::actions::{Actions, Panel};
use crate::dialogs::dialogs;
use crate::dock::dock;
use crate::extensions::extensions_page;
use crate::history::history_page;
use crate::overview::overview_page;
use crate::quick_input::quick_input;
use crate::session::session_view;
use crate::side_panel::side_panel;
use crate::sidebar::sidebar;
use crate::top_bar::top_bar;
use crate::welcome::welcome_page;

/// Emits the page `workspace` shows, the welcome page while new work is
/// being started, calling `actions` when the person acts.
fn page(workspace: &Workspace, actions: &Actions) {
    if let Some(start) = &workspace.starting {
        welcome_page(workspace, start, actions);
        return;
    }
    match &workspace.page {
        Page::Home | Page::Session(_) => session_view(workspace, actions),
        Page::History => history_page(workspace, actions),
        Page::Extensions => extensions_page(workspace, actions),
        Page::Overview(path) => overview_page(workspace, path, actions),
    }
}

/// Emits the sash that resizes `panel`, telling `actions` its new size.
fn sash(workspace: &Workspace, actions: &Actions, panel: Panel) {
    let sizes = workspace.sizes;
    let resize = actions.resize.clone();
    let sash = match panel {
        Panel::Sidebar => {
            Sash::new(sizes.sidebar).range(PanelSizes::SIDEBAR_MIN, PanelSizes::SIDEBAR_MAX)
        }
        Panel::Right => Sash::new(sizes.right)
            .range(PanelSizes::RIGHT_MIN, PanelSizes::RIGHT_MAX)
            .after(),
        Panel::Dock => Sash::new(sizes.dock)
            .variant(SashVariant::Horizontal)
            .range(PanelSizes::DOCK_MIN, PanelSizes::DOCK_MAX)
            .after(),
    };
    key(panel_key(panel), || {
        sash.on_resize(move |size| resize(panel, size)).show()
    });
}

/// The key that keeps each panel's sash its own drag state.
fn panel_key(panel: Panel) -> &'static str {
    match panel {
        Panel::Sidebar => "sidebar-sash",
        Panel::Right => "right-sash",
        Panel::Dock => "dock-sash",
    }
}

/// Emits the window for `workspace` in `theme`, calling `actions` when the person acts.
pub fn workspace_page(workspace: &Workspace, theme: AppTheme, actions: &Actions) {
    app_theme(theme, || {
        let scheme = color_scheme();
        stack_with(
            Modifier::new()
                .fill_max_size()
                .on_preview_key_event(shortcuts(actions)),
            || {
                column_with(
                    Modifier::new()
                        .fill_max_size()
                        .background(scheme.background),
                    || {
                        top_bar(workspace, actions);
                        row_with(Modifier::new().fill_max_width().weight(1.0), || {
                            if workspace.sidebar_open {
                                sidebar(workspace, actions);
                                sash(workspace, actions, Panel::Sidebar);
                            }
                            column_with(Modifier::new().weight(1.0).fill_max_height(), || {
                                column_with(Modifier::new().fill_max_width().weight(1.0), || {
                                    page(workspace, actions)
                                });
                                if workspace.dock_open {
                                    sash(workspace, actions, Panel::Dock);
                                    dock(workspace, actions);
                                }
                            });
                            if workspace.right_open {
                                sash(workspace, actions, Panel::Right);
                                side_panel(workspace, actions);
                            }
                        });
                    },
                );
                overlays(workspace, actions);
                dialogs(workspace, actions);
            },
        );
    });
}

/// The modals over the window: the quick input, and the question before an entry of the file tree is deleted.
fn overlays(workspace: &Workspace, actions: &Actions) {
    quick_input(workspace, actions);
    if let Some(path) = &workspace.tree.removing {
        let cancel = actions.tree.clone();
        let dismiss = actions.tree.clone();
        let remove = actions.tree.clone();
        let target = path.clone();
        Modal::new()
            .size(Size::Sm)
            .title(format!("Delete {}?", name_of(path)))
            .message(format!(
                "{path} and everything in it is taken off the disk. This cannot be undone."
            ))
            .show(
                move || dismiss(TreeAction::Cancel),
                || {
                    row_arranged(
                        Modifier::new().fill_max_width().padding_xy(0.0, 4.0),
                        Arrangement::End,
                        Alignment::Center,
                        || {
                            Button::new("Cancel")
                                .variant(ButtonVariant::Secondary)
                                .on_click(move || cancel(TreeAction::Cancel))
                                .show();
                            spacer(Modifier::new().width(8.0));
                            Button::new("Delete")
                                .variant(ButtonVariant::Danger)
                                .icon(phosphor::TRASH)
                                .on_click(move || remove(TreeAction::Remove(target.clone())))
                                .show();
                        },
                    );
                },
            );
    }
}

/// The window's keys: each chord pressed anywhere in the window is handed to the keymap the settings make, which takes those bound to actions that work everywhere.
fn shortcuts(actions: &Actions) -> impl Fn(&KeyEvent) -> bool + 'static {
    let shortcut = actions.shortcut.clone();
    move |event| event.pressed && KeyRecorder::chord(event).is_some_and(|chord| shortcut(chord))
}

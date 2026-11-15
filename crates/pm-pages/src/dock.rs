//! The bottom dock: Problems, Terminal, Tasks and Debug tabs over their
//! content, with the shells of the folder as tabs of their own while the
//! terminal shows.

use rustle::prelude::*;

use pm_components::{IconButton, Label, LabelVariant, Size, Tab, TerminalView};
use pm_shell::{DockTab, ProblemSeverity, Workspace};

use crate::actions::{Actions, Panel};
use crate::debug_panel::debug_panel;
use crate::problems_panel::problems_panel;
use crate::tasks_panel::tasks_panel;

/// Emits the dock for `workspace`.
pub fn dock(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    column_with(
        Modifier::new()
            .fill_max_width()
            .height(workspace.sizes.dock)
            .background(scheme.background),
        || {
            tab_bar(workspace, actions);
            column_with(
                Modifier::new().fill_max_size().padding_xy(12.0, 4.0),
                || match workspace.dock_tab {
                    DockTab::Terminal => terminal(workspace, actions),
                    DockTab::Problems => {
                        problems_panel(&workspace.problems, actions.open_location.clone())
                    }
                    DockTab::Tasks => tasks_panel(&workspace.tasks, actions),
                    DockTab::Debug => debug_panel(&workspace.debug, actions),
                },
            );
        },
    );
}

/// The number the Problems tab shows in its badge: the problems that are not hints, when there are any.
fn problem_count(tab: DockTab, workspace: &Workspace) -> Option<usize> {
    let counted = workspace
        .problems
        .iter()
        .filter(|problem| problem.severity != ProblemSeverity::Hint)
        .count();
    (tab == DockTab::Problems && counted > 0).then_some(counted)
}

/// What the tab of `tab` reads.
fn tab_label(tab: DockTab) -> String {
    match tab {
        DockTab::Problems => "Problems".to_owned(),
        DockTab::Terminal => "Terminal".to_owned(),
        DockTab::Tasks => "Tasks".to_owned(),
        DockTab::Debug => "Debug".to_owned(),
    }
}

/// The row of the dock's tabs, then the shells' tabs while the terminal shows, and the dock's buttons.
fn tab_bar(workspace: &Workspace, actions: &Actions) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(pm_components::row_height(40.0))
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(2.0)),
        Alignment::Center,
        || {
            for tab in [
                DockTab::Problems,
                DockTab::Terminal,
                DockTab::Tasks,
                DockTab::Debug,
            ] {
                key(format!("{tab:?}"), || {
                    let choose = actions.dock_tab.clone();
                    Tab::new(tab_label(tab))
                        .count(problem_count(tab, workspace))
                        .size(Size::Sm)
                        .selected(workspace.dock_tab == tab)
                        .on_click(move || choose(tab))
                        .show();
                });
            }
            if workspace.dock_tab == DockTab::Terminal {
                let strip = remember_scroll_state();
                row_arranged(
                    Modifier::new()
                        .weight(1.0)
                        .fill_max_height()
                        .padding_xy(8.0, 0.0)
                        .horizontal_scroll(&strip),
                    Arrangement::SpacedBy(pm_components::spacing(2.0)),
                    Alignment::Center,
                    || shell_tabs(workspace, actions),
                );
                let new = actions.terminal_new.clone();
                IconButton::new(phosphor::PLUS)
                    .size(Size::Xs)
                    .on_click(move || new())
                    .show();
            } else {
                spacer(Modifier::new().weight(1.0));
            }
            if matches!(workspace.dock_tab, DockTab::Tasks | DockTab::Debug) {
                let refresh = actions.refresh_dock.clone();
                IconButton::new(phosphor::ARROW_CLOCKWISE)
                    .size(Size::Xs)
                    .on_click(move || refresh())
                    .show();
            }
            let toggle = actions.toggle.clone();
            IconButton::new(phosphor::X)
                .size(Size::Xs)
                .on_click(move || toggle(Panel::Dock))
                .show();
        },
    );
}

/// A tab for each shell of the folder, with a button that ends it.
fn shell_tabs(workspace: &Workspace, actions: &Actions) {
    for shell in &workspace.terminals {
        key(shell.key, || {
            let pick = actions.terminal_pick.clone();
            let close = actions.terminal_close.clone();
            let shell_key = shell.key;
            let glyph = if shell.task {
                phosphor::PLAY
            } else {
                phosphor::TERMINAL_WINDOW
            };
            let title = if shell.exited {
                format!("{} (ended)", shell.title)
            } else {
                shell.title.clone()
            };
            Tab::new(title)
                .icon(glyph)
                .size(Size::Xs)
                .selected(workspace.terminal_shown == Some(shell_key))
                .on_click(move || pick(shell_key))
                .on_close(move || close(shell_key))
                .show();
        });
    }
}

/// The shown shell's screen, or a line saying none runs.
fn terminal(workspace: &Workspace, actions: &Actions) {
    if workspace.terminal_shown.is_none() {
        Label::new("No shell is running. Press + to start one.")
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        return;
    }
    let input = actions.terminal_input.clone();
    let resize = actions.terminal_resize.clone();
    let scroll = actions.terminal_scroll.clone();
    let select = actions.terminal_select.clone();
    let command = actions.terminal_command.clone();
    let screen = &workspace.terminal;
    TerminalView::new(screen.lines.clone())
        .cursor(screen.cursor)
        .selection(screen.selection)
        .application_cursor(screen.application_cursor)
        .size(Size::Sm)
        .focus_key(workspace.terminal_shown)
        .modifier(Modifier::new().fill_max_size())
        .on_input(move |bytes| input(bytes))
        .on_resize(move |columns, rows| resize(columns, rows))
        .on_scroll(move |lines| scroll(lines))
        .on_select(move |selection| select(selection))
        .on_command(move |chosen| command(chosen))
        .show();
}

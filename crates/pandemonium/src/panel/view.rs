//! The bottom panel as it is drawn: the bar of views, and the one in front.
//!
//! The bar along the top names every view and lights the chosen one, with
//! the view's own actions and the control that closes the panel at its far
//! end. The terminal view is the shell in front beside the list of every
//! shell the worktree is running, once there is more than one to list.

use pm_ui::{
    Bounds, Div, IconName, IconSize, Scrolled, Styled, Theme, h_flex, icon, icon_button, rule,
    text, v_flex, view_tab,
};

use crate::message::Message;
use crate::panel::PanelView;
use crate::panel::problems::{ProblemFile, badge, problems_view};
use crate::terminal::{Shell, ShellEntry, terminal_view};

/// How wide the list of shells beside the terminal is.
const SHELL_LIST_WIDTH: f32 = 180.0;

/// What the bottom panel is showing, and what each of its views holds.
///
/// Only the view in front is filled in: the debugger is built by the window
/// that knows whether its console has the keyboard, and a shell is started
/// only once the terminal view is looked at.
pub struct Panel {
    /// Which view is in front.
    pub view: PanelView,
    /// The shell the terminal view draws, when one is running.
    pub shell: Option<Shell>,
    /// Every shell of the worktree, for the list beside it.
    pub shells: Vec<ShellEntry>,
    /// Whether keystrokes are going to the shell.
    pub focused: bool,
    /// Whether the key that follows a link is held.
    pub linking: bool,
    /// The open files with problems, and what the problems are.
    pub problems: Vec<ProblemFile>,
    /// How far the list of problems is scrolled.
    pub problems_scroll: Scrolled,
    /// Where the list of problems came out in the last frame.
    pub problems_area: Bounds,
    /// The debugger, when that is the view in front.
    pub debug: Option<Div<Message>>,
}

/// Builds the bottom panel, `height` tall.
pub fn bottom_panel(theme: &Theme, height: f32, panel: Panel) -> Div<Message> {
    let count = panel.problems.iter().map(|file| file.problems.len()).sum();
    let view = panel.view;

    v_flex()
        .w_full()
        .h_px(height)
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(bar(theme, view, count))
        .child(
            v_flex()
                .w_full()
                .flex_1()
                .overflow_hidden()
                .child(match view {
                    PanelView::Problems => problems_view(
                        theme,
                        &panel.problems,
                        panel.problems_scroll,
                        panel.problems_area,
                    ),
                    PanelView::Debug => panel.debug.unwrap_or_else(v_flex),
                    PanelView::Terminal => terminals(theme, panel),
                }),
        )
}

/// Builds the bar: one label per view, then the view's own actions, standing
/// on the hairline that parts it from the view.
fn bar(theme: &Theme, view: PanelView, problems: usize) -> Div<Message> {
    let actions = h_flex()
        .h_full()
        .px(1.5)
        .gap(1)
        .items_center()
        .when(view == PanelView::Terminal, |actions| {
            actions.child(icon_button(theme, IconName::Plus, Message::NewTerminal))
        })
        .child(icon_button(
            theme,
            IconName::Close,
            Message::ToggleBottomPanel,
        ));

    v_flex()
        .w_full()
        .h_px(theme.size.tab_bar)
        .bg(theme.colors.surface)
        .child(
            h_flex()
                .w_full()
                .flex_1()
                .pl(1)
                .items_stretch()
                .children(PanelView::ALL.map(|offered| {
                    let count =
                        (offered == PanelView::Problems && problems > 0).then_some(problems);
                    view_tab(
                        theme,
                        offered.label(),
                        offered == view,
                        count.map(|count| badge(theme, count)),
                        Message::ShowPanelView(offered),
                    )
                }))
                .child(h_flex().flex_1())
                .child(actions),
        )
        .child(rule(theme))
}

/// Builds the terminal view: the shell in front, and the list of them.
fn terminals(theme: &Theme, panel: Panel) -> Div<Message> {
    let screen = match panel.shell {
        Some(shell) => v_flex().flex_1().h_full().overflow_hidden().child(
            terminal_view(shell, panel.focused)
                .linking(panel.linking)
                .on_point(Message::PointTerminal)
                .on_menu(Message::ShowScreenMenu)
                .on_scroll(Message::ScrollTerminal),
        ),
        None => v_flex().flex_1().h_full().child(
            text("No shell is running in this worktree")
                .text_sm()
                .font_light()
                .color(theme.colors.text_subtle)
                .px(2)
                .py(1.5),
        ),
    };

    h_flex()
        .w_full()
        .flex_1()
        .overflow_hidden()
        .child(screen)
        .when(panel.shells.len() > 1, |row| {
            row.child(v_flex().w_px(1.0).h_full().bg(theme.colors.border))
                .child(
                    v_flex()
                        .w_px(SHELL_LIST_WIDTH)
                        .h_full()
                        .p(0.5)
                        .gap(0.25)
                        .overflow_hidden()
                        .children(panel.shells.iter().map(|shell| shell_row(theme, shell))),
                )
        })
}

/// Builds one shell's row in the list: what it runs, and the control that
/// ends it.
fn shell_row(theme: &Theme, shell: &ShellEntry) -> Div<Message> {
    let color = match shell.active {
        true => theme.colors.text,
        false => theme.colors.text_muted,
    };
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pl(1)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .rounded(theme.radius.md)
        .when(shell.active, |row| row.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::SelectTerminal(shell.id))
        .on_secondary_click(Message::ShowTerminalMenu(shell.id))
        .child(icon(IconName::Terminal).size(IconSize::Small).color(color))
        .child(text(shell.name.clone()).text_sm().font_light().color(color))
        .child(h_flex().flex_1())
        .child(icon_button(
            theme,
            IconName::Close,
            Message::CloseTerminal(shell.id),
        ))
}

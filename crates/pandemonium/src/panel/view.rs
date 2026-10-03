//! Content for the Problems, Debug Console and Terminal tool tabs.

use pm_ui::{
    Bounds, Div, IconName, IconSize, Scrolled, Styled, Theme, h_flex, icon, icon_button, text,
    v_flex,
};

use crate::message::Message;
use crate::panel::PanelView;
use crate::panel::problems::{ProblemFile, problems_view};
use crate::terminal::{Shell, ShellEntry, terminal_view};

/// How wide the list of shells beside the terminal is.
const SHELL_LIST_WIDTH: f32 = 180.0;

/// The content and input state of one worktree tool view.
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

/// Builds the content of a worktree tool beneath its ordinary pane tabs.
pub fn panel_content(theme: &Theme, panel: Panel) -> Div<Message> {
    match panel.view {
        PanelView::Problems => problems_view(
            theme,
            &panel.problems,
            panel.problems_scroll,
            panel.problems_area,
        ),
        PanelView::Debug => panel.debug.unwrap_or_else(v_flex),
        PanelView::Terminal => v_flex()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .child(h_flex().w_full().justify_end().child(
                icon_button(theme, IconName::Plus, Message::NewTerminal).tooltip("New Terminal"),
            ))
            .child(terminals(theme, panel)),
    }
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

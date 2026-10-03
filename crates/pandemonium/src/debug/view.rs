//! The program being debugged, in the bottom panel: where it stands, and
//! what it holds.
//!
//! Along the top is what is being debugged and the controls that run it on —
//! continue or pause, the three steps, restart and stop — in the order every
//! debugger puts them. Beneath, side by side because the panel is wide and
//! short, are the paused thread's stack, what the selected frame's variables
//! hold, opened and closed like a file tree, and the console, where the
//! program's output lands and where an expression is typed to be asked about.
//!
//! The panel is as tall as the reader made it, so the console shows as many
//! of its last lines as the last frame had room for, and the wheel walks back
//! through the rest.

use std::path::Path;

use pm_dap::{Category, Frame, Line, Session, Standing, Variable};
use pm_gfx::Rgba;
use pm_ui::{
    Div, IconName, IconSize, Styled, Theme, button, h_flex, icon, icon_button, measured, rule,
    scroll_area, text, tinted_icon_button, v_flex,
};

use crate::debug::Debugger;
use crate::input::input_view;
use crate::keymap::Action;
use crate::message::Message;

/// How far one level of a variable's members is indented.
const INDENT: f32 = 1.25;

/// How deep the variables tree is drawn, however far it has been opened.
const DEEPEST: usize = 12;

/// How wide the call stack is.
const STACK_WIDTH: f32 = 280.0;

/// How wide the variables are.
const VARIABLES_WIDTH: f32 = 360.0;

/// Builds the view showing `debugger`, or offering to start one where there
/// is none; `typing` says its console has the keyboard.
pub fn debug_view(
    theme: &Theme,
    debugger: Option<&Debugger>,
    typing: bool,
    solid: bool,
) -> Div<Message> {
    let Some(debugger) = debugger else {
        return idle(theme);
    };
    let session = debugger.session();

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(header(theme, session))
        .child(rule(theme))
        .child(
            h_flex()
                .w_full()
                .flex_1()
                .overflow_hidden()
                .child(
                    v_flex()
                        .w_px(STACK_WIDTH)
                        .h_full()
                        .overflow_hidden()
                        .child(heading(theme, "CALL STACK"))
                        .child(measured(
                            debugger.stack_area(),
                            scroll_area(debugger.stack_scroll(), stack(theme, session))
                                .selectable()
                                .w_full()
                                .flex_1(),
                        )),
                )
                .child(divider(theme))
                .child(
                    v_flex()
                        .w_px(VARIABLES_WIDTH)
                        .h_full()
                        .overflow_hidden()
                        .child(heading(theme, "VARIABLES"))
                        .child(measured(
                            debugger.variables_area(),
                            scroll_area(debugger.variables_scroll(), variables(theme, debugger))
                                .selectable()
                                .w_full()
                                .flex_1(),
                        )),
                )
                .child(divider(theme))
                .child(console(theme, debugger, typing, solid)),
        )
}

/// Builds the line standing between two of the view's columns.
fn divider(theme: &Theme) -> Div<Message> {
    v_flex().w_px(1.0).h_full().bg(theme.colors.border)
}

/// Builds what the view says while nothing is being debugged.
fn idle(theme: &Theme) -> Div<Message> {
    v_flex()
        .w_full()
        .h_full()
        .gap(2)
        .items_center()
        .justify_center()
        .bg(theme.colors.background)
        .child(icon(IconName::Debug).size(IconSize::Medium))
        .child(
            text("Nothing is being debugged in this worktree")
                .text_sm()
                .color(theme.colors.text_subtle),
        )
        .child(button(
            "Start Debugging",
            Message::ActOnDebugger(Action::DebugStart),
        ))
}

/// Builds the bar along the top: what is being debugged, where it stands,
/// and the controls that run it.
fn header(theme: &Theme, session: &Session) -> Div<Message> {
    let standing = session.standing();
    let scenario = session.scenario();
    let adapter = scenario.adapter.map_or("", |adapter| adapter.name);

    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(1.5)
        .gap(1)
        .items_center()
        .bg(theme.colors.surface)
        .child(text("●").text_xs().color(standing_color(theme, standing)))
        .child(
            text(scenario.label.clone())
                .text_xs()
                .color(theme.colors.text_muted),
        )
        .child(chip(theme, adapter))
        .child(chip(theme, said(standing, session.reason())))
        .child(h_flex().flex_1())
        .child(controls(theme, standing))
}

/// Builds the controls that run the program, as they apply where it stands.
fn controls(theme: &Theme, standing: Standing) -> Div<Message> {
    let stopped = standing == Standing::Stopped;
    let live = standing != Standing::Ended;
    let step =
        |name, action, tip: &str| control(theme, name, action, stopped).tooltip(tip.to_owned());

    h_flex()
        .gap(0.5)
        .items_center()
        .child(match stopped {
            true => control(theme, IconName::DebugContinue, Action::DebugContinue, true)
                .tooltip("Continue"),
            false => {
                control(theme, IconName::DebugPause, Action::DebugPause, live).tooltip("Pause")
            }
        })
        .child(step(
            IconName::DebugStepOver,
            Action::DebugStepOver,
            "Step Over",
        ))
        .child(step(
            IconName::DebugStepInto,
            Action::DebugStepInto,
            "Step Into",
        ))
        .child(step(
            IconName::DebugStepOut,
            Action::DebugStepOut,
            "Step Out",
        ))
        .child(control(theme, IconName::Restart, Action::DebugRestart, true).tooltip("Restart"))
        .child(control(theme, IconName::Stop, Action::DebugStop, live).tooltip("Stop"))
}

/// Builds one control, lit when it applies and quiet when it does not.
fn control(theme: &Theme, name: IconName, action: Action, applies: bool) -> Div<Message> {
    let message = Message::ActOnDebugger(action);
    match applies {
        true => icon_button(theme, name, message),
        false => tinted_icon_button(theme, name, theme.colors.border, message),
    }
}

/// What the header says the session is doing.
fn said(standing: Standing, reason: Option<String>) -> String {
    match standing {
        Standing::Starting => "Starting".to_owned(),
        Standing::Running => "Running".to_owned(),
        Standing::Stopped => match reason {
            Some(reason) => format!("Paused on {reason}"),
            None => "Paused".to_owned(),
        },
        Standing::Ended => "Ended".to_owned(),
    }
}

/// The colour of the dot that says where the session stands.
fn standing_color(theme: &Theme, standing: Standing) -> Rgba {
    match standing {
        Standing::Starting | Standing::Ended => theme.colors.text_subtle,
        Standing::Running => theme.colors.success,
        Standing::Stopped => theme.colors.warning,
    }
}

/// Builds one of the header's chips.
fn chip(theme: &Theme, label: impl Into<String>) -> Div<Message> {
    h_flex()
        .px(0.5)
        .items_center()
        .rounded(theme.radius.sm)
        .border_1(theme.colors.border)
        .child(
            text(label.into())
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
}

/// Builds the heading over one of the view's lists.
fn heading(theme: &Theme, title: &str) -> Div<Message> {
    h_flex().w_full().px(1.5).py(0.5).child(
        text(title.to_owned())
            .text_xs()
            .font_medium()
            .color(theme.colors.text_subtle),
    )
}

/// Builds the paused thread's stack, the selected frame lit.
fn stack(theme: &Theme, session: &Session) -> Div<Message> {
    let frames = session.frames();
    let selected = session.frame().map(|frame| frame.id);
    let root = session.root();
    if frames.is_empty() {
        return v_flex().w_full().child(note(
            theme,
            match session.standing() {
                Standing::Running => "Running…",
                _ => "Not paused",
            },
        ));
    }
    v_flex().w_full().children(
        frames
            .iter()
            .map(|frame| frame_row(theme, frame, selected == Some(frame.id), root)),
    )
}

/// Builds one frame of the stack: the function, and where it is.
fn frame_row(theme: &Theme, frame: &Frame, selected: bool, root: &Path) -> Div<Message> {
    let place = frame
        .path
        .as_deref()
        .map(|path| format!("{}:{}", shown(path, root), frame.line + 1))
        .unwrap_or_default();
    h_flex()
        .w_full()
        .px(1.5)
        .py(0.25)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .hover_bg(theme.colors.surface_hover)
        .when(selected, |row| row.bg(theme.colors.surface_selected))
        .on_click(Message::SelectFrame(frame.id))
        .child(
            text(frame.name.clone())
                .text_xs()
                .font_mono()
                .color(theme.syntax.function),
        )
        .child(
            text(place)
                .text_xs()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the selected frame's scopes, and whatever of them is open.
fn variables(theme: &Theme, debugger: &Debugger) -> Div<Message> {
    let session = debugger.session();
    let scopes = session.scopes();
    let mut rows = Vec::new();
    let open = debugger.is_scope_open("Watch");
    rows.push(
        tree_row(theme, 0, Some(open))
            .on_click(Message::ToggleWatchSection)
            .child(
                text("Watch")
                    .text_xs()
                    .font_medium()
                    .color(theme.colors.text_muted),
            ),
    );
    if open {
        let watched = session.watched();
        for (index, expression) in session.watches().iter().enumerate() {
            let result = watched
                .iter()
                .find(|watched| watched.expression == *expression);
            let value = result.map(|watched| &watched.value);
            let reference = value
                .and_then(|value| value.as_ref().ok())
                .map_or(0, |value| value.reference);
            let expanded = reference != 0 && debugger.is_open(reference);
            let color = if session.standing() == Standing::Running {
                theme.colors.text_subtle
            } else {
                theme.colors.text
            };
            let mut row = tree_row(theme, 1, (reference != 0).then_some(expanded))
                .child(
                    h_flex().on_click(Message::EditWatch(index)).child(
                        text(expression.clone())
                            .text_xs()
                            .font_mono()
                            .color(theme.syntax.property),
                    ),
                )
                .child(quiet(theme, "="));
            row = match value {
                Some(Ok(variable)) => row
                    .child(
                        text(first_line(&variable.value))
                            .text_xs()
                            .font_mono()
                            .color(color),
                    )
                    .when_some(variable.kind.clone(), |row, kind| {
                        row.child(quiet(theme, &kind))
                    }),
                Some(Err(error)) => {
                    row.child(text(error.clone()).text_xs().color(theme.colors.danger))
                }
                None => row.child(quiet(theme, "loading…")),
            };
            if reference != 0 {
                row = row.on_click(Message::ToggleVariable(reference));
            }
            row = row.child(h_flex().flex_1()).child(
                icon_button(theme, IconName::Close, Message::RemoveWatch(index))
                    .tooltip("Remove watch"),
            );
            rows.push(row);
            if expanded {
                members(theme, debugger, reference, 2, &mut rows);
            }
        }
        rows.push(
            tree_row(theme, 1, None)
                .on_click(Message::ActOnDebugger(Action::DebugAddWatch))
                .child(quiet(theme, "Add Watch…")),
        );
    }
    if scopes.is_empty() && session.watches().is_empty() {
        rows.push(note(theme, "Nothing to show"));
    }
    for (place, scope) in scopes.iter().enumerate() {
        let open = debugger.is_scope_open(&scope.name);
        rows.push(
            tree_row(theme, 0, Some(open))
                .on_click(Message::ToggleDebugScope(place))
                .child(
                    text(scope.name.clone())
                        .text_xs()
                        .font_medium()
                        .color(theme.colors.text_muted),
                ),
        );
        if open {
            members(theme, debugger, scope.reference, 1, &mut rows);
        }
    }
    v_flex().w_full().children(rows)
}

/// Adds the rows for what `reference` holds, `depth` levels in.
fn members(
    theme: &Theme,
    debugger: &Debugger,
    reference: i64,
    depth: usize,
    rows: &mut Vec<Div<Message>>,
) {
    let Some(variables) = debugger.session().variables(reference) else {
        rows.push(tree_row(theme, depth, None).child(quiet(theme, "loading…")));
        return;
    };
    for variable in &variables {
        let opens = variable.reference != 0;
        let open = opens && debugger.is_open(variable.reference);
        rows.push(variable_row(theme, variable, depth, opens.then_some(open)));
        if open && depth < DEEPEST {
            members(theme, debugger, variable.reference, depth + 1, rows);
        }
    }
}

/// Builds one variable's row: its name, what it holds and its type.
fn variable_row(
    theme: &Theme,
    variable: &Variable,
    depth: usize,
    open: Option<bool>,
) -> Div<Message> {
    let row = tree_row(theme, depth, open)
        .child(
            text(variable.name.clone())
                .text_xs()
                .font_mono()
                .color(theme.syntax.property),
        )
        .child(quiet(theme, "="))
        .child(
            text(first_line(&variable.value))
                .text_xs()
                .font_mono()
                .color(theme.colors.text),
        )
        .when_some(variable.kind.clone(), |row, kind| {
            row.child(quiet(theme, &kind))
        });
    match open {
        Some(_) => row.on_click(Message::ToggleVariable(variable.reference)),
        None => row,
    }
}

/// Builds a row of the variables tree, indented `depth` levels, with a
/// chevron when it opens: pointing down when `open`, right when not.
fn tree_row(theme: &Theme, depth: usize, open: Option<bool>) -> Div<Message> {
    let chevron = match open {
        Some(true) => icon(IconName::ChevronDown).size(IconSize::XSmall),
        Some(false) => icon(IconName::ChevronRight).size(IconSize::XSmall),
        None => icon(IconName::ChevronRight)
            .size(IconSize::XSmall)
            .color(Rgba::TRANSPARENT),
    };
    h_flex()
        .w_full()
        .pl(1.5 + depth as f32 * INDENT)
        .pr(1.5)
        .py(0.25)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .hover_bg(theme.colors.surface_hover)
        .child(chevron)
}

/// Builds the console: its last lines, and the box an expression is typed in.
fn console(theme: &Theme, debugger: &Debugger, typing: bool, solid: bool) -> Div<Message> {
    let lines = debugger.session().lines();

    v_flex()
        .flex_1()
        .h_full()
        .overflow_hidden()
        .child(heading(theme, "CONSOLE"))
        .child(measured(
            debugger.console_area(),
            scroll_area(
                debugger.console_scroll(),
                v_flex()
                    .w_full()
                    .px(1.5)
                    .py(0.5)
                    .children(lines.iter().map(|line| console_line(theme, line))),
            )
            .selectable()
            .w_full()
            .flex_1(),
        ))
        .child(v_flex().w_full().px(1.25).pb(1).child(input_view(
            theme,
            debugger.console(),
            typing,
            solid,
            1.0,
            Message::WriteDebugConsole,
            Message::ShowInputMenu,
        )))
}

/// Builds one line of the console, coloured by who it is from.
fn console_line(theme: &Theme, line: &Line) -> Div<Message> {
    let (prefix, color) = match line.category {
        Category::Stdout => ("", theme.colors.text),
        Category::Stderr | Category::Error => ("", theme.colors.danger),
        Category::Console => ("", theme.colors.text_muted),
        Category::Asked => ("> ", theme.colors.text_muted),
        Category::Answer => ("< ", theme.colors.text),
    };
    h_flex().child(
        text(format!("{prefix}{}", line.text))
            .text_xs()
            .font_mono()
            .color(color),
    )
}

/// Builds a quiet line saying there is nothing to list.
fn note(theme: &Theme, said: &str) -> Div<Message> {
    h_flex().w_full().px(1.5).py(0.5).child(quiet(theme, said))
}

/// Quiet text, for what stands beside the thing a row is about.
fn quiet(theme: &Theme, said: &str) -> pm_ui::Text {
    text(said.to_owned())
        .text_xs()
        .font_mono()
        .color(theme.colors.text_subtle)
}

/// The first line of `value`, which is as much as a row has room for.
fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or_default().to_owned()
}

/// `path` from the worktree down, or whole where it is outside it.
fn shown(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

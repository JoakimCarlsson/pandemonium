//! The dock's Debug tab: what the folder can be debugged as, then the
//! session followed with its controls, stack, variables, watches and
//! console, beside the breakpoints set.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    IconButton, Label, LabelVariant, Leading, ListRow, ListRowVariant, Size, TextInput,
};
use pm_shell::{DebugCommand, DebugModel, DebugSession, DebugStanding, DebugVariableEntry};

use crate::actions::Actions;

/// The share of the tab's width the column of the stack takes.
const STACK_SHARE: f32 = 0.32;

/// The share of the tab's width the column of the variables, or of the scenarios, takes.
const VARIABLES_SHARE: f32 = 0.33;

/// The share of the tab's width the column of the breakpoints, watches and console takes.
const SIDE_SHARE: f32 = 0.35;

/// Emits the Debug tab for `model`.
pub fn debug_panel(model: &DebugModel, actions: &Actions) {
    row_arranged(
        Modifier::new().fill_max_size(),
        Arrangement::SpacedBy(pm_components::spacing(12.0)),
        Alignment::Start,
        || {
            match &model.session {
                Some(session) => session_columns(session, actions),
                None => scenarios(model, actions),
            }
            side_column(model, actions);
        },
    );
}

/// A scrolling column laid out with `modifier`, holding `content`.
#[track_caller]
fn scrolling(modifier: Modifier, content: impl FnOnce()) {
    let scroll = remember_scroll_state();
    column_with(modifier.fill_max_height().vertical_scroll(&scroll), content);
}

/// The scenarios the folder can be debugged as, each starting a session when pressed.
fn scenarios(model: &DebugModel, actions: &Actions) {
    scrolling(
        Modifier::new().weight(STACK_SHARE + VARIABLES_SHARE),
        || {
            if let Some(error) = &model.error {
                Label::new(error.clone())
                    .colour(color_scheme().error)
                    .size(Size::Xs)
                    .show();
            }
            if !model.loaded {
                Label::new("Open a session to debug its folder.")
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
                return;
            }
            if model.scenarios.is_empty() {
                Label::new("Nothing in the folder can be debugged.")
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
            }
            for (index, scenario) in model.scenarios.iter().enumerate() {
                key(index, || {
                    let start = actions.debug_start.clone();
                    let note = if scenario.installed {
                        scenario.adapter.clone()
                    } else {
                        format!("{} (not installed)", scenario.adapter)
                    };
                    ListRow::new(scenario.label.clone())
                        .leading(Leading::Icon(phosphor::BUG))
                        .note(note)
                        .variant(if scenario.installed {
                            ListRowVariant::Plain
                        } else {
                            ListRowVariant::Dimmed
                        })
                        .size(Size::Xs)
                        .on_click(move || start(index))
                        .show();
                });
            }
        },
    );
}

/// The session's controls over its stack, and its variables beside them.
fn session_columns(session: &DebugSession, actions: &Actions) {
    scrolling(Modifier::new().weight(STACK_SHARE), || {
        controls(session, actions);
        heading("Call stack");
        for frame in &session.frames {
            key(frame.id, || {
                let choose = actions.debug_command.clone();
                let id = frame.id;
                let looked_at = session.frame == Some(id);
                ListRow::new(frame.name.clone())
                    .leading(if looked_at {
                        Leading::Icon(phosphor::CARET_RIGHT)
                    } else {
                        Leading::None
                    })
                    .end_column(frame.place.clone(), 90.0)
                    .size(Size::Xs)
                    .on_click(move || choose(DebugCommand::SelectFrame(id)))
                    .show();
            });
        }
    });
    scrolling(Modifier::new().weight(VARIABLES_SHARE), || {
        heading("Variables");
        if session.scopes.is_empty() {
            Label::new(match session.standing {
                DebugStanding::Stopped => "The frame has no variables.",
                _ => "Variables show while the program is paused.",
            })
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        }
        for scope in &session.scopes {
            key(scope.reference, || {
                Label::new(scope.name.clone())
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .show();
                for (index, variable) in scope.variables.iter().enumerate() {
                    key(index, || variable_row(variable, actions));
                }
            });
        }
    });
}

/// A muted heading over a column of the tab, reading `title`.
fn heading(title: &str) {
    Label::new(title)
        .variant(LabelVariant::Muted)
        .size(Size::Xs)
        .modifier(Modifier::new().padding_xy(4.0, 4.0))
        .show();
}

/// The buttons that drive the session, each with a tooltip and enabled only while the program stands where it applies, and how it stands.
fn controls(session: &DebugSession, actions: &Actions) {
    row_arranged(
        Modifier::new().fill_max_width().height(28.0),
        Arrangement::SpacedBy(pm_components::spacing(2.0)),
        Alignment::Center,
        || {
            let running = session.standing == DebugStanding::Running;
            let paused = session.standing == DebugStanding::Stopped;
            let ended = session.standing == DebugStanding::Ended;
            let buttons = [
                if running {
                    (phosphor::PAUSE, DebugCommand::Pause, "Pause", true)
                } else {
                    (phosphor::PLAY, DebugCommand::Continue, "Continue", paused)
                },
                (
                    phosphor::ARROW_BEND_DOWN_RIGHT,
                    DebugCommand::StepOver,
                    "Step over",
                    paused,
                ),
                (
                    phosphor::ARROW_DOWN,
                    DebugCommand::StepIn,
                    "Step into",
                    paused,
                ),
                (
                    phosphor::ARROW_UP,
                    DebugCommand::StepOut,
                    "Step out",
                    paused,
                ),
                (phosphor::STOP, DebugCommand::Stop, "Stop", true),
            ];
            if !ended {
                for (glyph, command, tip, enabled) in buttons {
                    let send = actions.debug_command.clone();
                    IconButton::new(glyph)
                        .size(Size::Xs)
                        .tooltip(tip)
                        .disabled(!enabled)
                        .on_click(move || {
                            if enabled {
                                send(command.clone())
                            }
                        })
                        .show();
                }
            }
            let standing = match session.standing {
                DebugStanding::Starting => "Starting".to_owned(),
                DebugStanding::Running => "Running".to_owned(),
                DebugStanding::Stopped => match &session.reason {
                    Some(reason) => format!("Paused: {reason}"),
                    None => "Paused".to_owned(),
                },
                DebugStanding::Ended => "Ended".to_owned(),
            };
            Label::new(format!("{} · {standing}", session.label))
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .show();
        },
    );
}

/// Emits one variable, indented by its depth, which shows or hides its members when pressed.
fn variable_row(variable: &DebugVariableEntry, actions: &Actions) {
    let depth = variable.depth;
    let caret = match (variable.reference, variable.expanded) {
        (0, _) => None,
        (_, true) => Some(phosphor::CARET_DOWN),
        (_, false) => Some(phosphor::CARET_RIGHT),
    };
    let leading = Leading::Custom(Rc::new(move || {
        let scheme = color_scheme();
        row_with(Modifier::new().padding_xy(depth as f32 * 12.0, 0.0), || {
            match &caret {
                Some(glyph) => icon(glyph.clone(), 12.0, scheme.on_surface_variant),
                None => spacer(Modifier::new().width(pm_components::spacing(12.0))),
            };
        });
    }));
    let mut row = ListRow::new(format!("{} = {}", variable.name, variable.value))
        .leading(leading)
        .size(Size::Xs);
    if let Some(kind) = &variable.kind {
        row = row.end_column(kind.clone(), 120.0);
    }
    if variable.reference != 0 {
        let expand = actions.debug_command.clone();
        let reference = variable.reference;
        row = row.on_click(move || expand(DebugCommand::Expand(reference)));
    }
    row.show();
}

/// The breakpoints and the watches over the console, then the box that evaluates an expression while a session runs.
fn side_column(model: &DebugModel, actions: &Actions) {
    column_with(Modifier::new().weight(SIDE_SHARE).fill_max_height(), || {
        let scroll = remember_scroll_state();
        column_with(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .clip()
                .vertical_scroll(&scroll),
            || {
                breakpoints(model, actions);
                if let Some(session) = &model.session {
                    watches(session, actions);
                }
            },
        );
        let Some(session) = &model.session else {
            return;
        };
        heading("Console");
        console(&session.console);
        if session.standing != DebugStanding::Ended {
            evaluate_box(actions);
        }
    });
}

/// The breakpoints set, each taken away when pressed.
fn breakpoints(model: &DebugModel, actions: &Actions) {
    Label::new("Breakpoints")
        .variant(LabelVariant::Muted)
        .size(Size::Xs)
        .show();
    if model.breakpoints.is_empty() {
        Label::new("No breakpoints are set.")
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
    }
    for breakpoint in &model.breakpoints {
        key((breakpoint.path.clone(), breakpoint.line), || {
            let toggle = actions.toggle_breakpoint.clone();
            let path = breakpoint.path.clone();
            let line = breakpoint.line;
            let tone = match breakpoint.verified {
                Some(false) => color_scheme().on_surface_variant,
                _ => color_scheme().error,
            };
            ListRow::new(format!("{}:{}", breakpoint.path, line + 1))
                .leading(Leading::Custom(Rc::new(move || {
                    icon(phosphor::CIRCLE, 12.0, tone)
                })))
                .end_column("Remove", 60.0)
                .size(Size::Xs)
                .on_click(move || toggle(path.clone(), line))
                .show();
        });
    }
}

/// The watch expressions of `session` with what each comes to and a button that takes it away, then a box that adds one.
fn watches(session: &DebugSession, actions: &Actions) {
    Label::new("Watches")
        .variant(LabelVariant::Muted)
        .size(Size::Xs)
        .show();
    for (expression, value) in &session.watches {
        key(expression.clone(), || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(pm_components::spacing(4.0)),
                Alignment::Center,
                || {
                    Label::new(format!("{expression} = {value}"))
                        .variant(LabelVariant::Code)
                        .size(Size::Xs)
                        .show();
                    spacer(Modifier::new().weight(1.0));
                    let remove = actions.debug_command.clone();
                    let watched = expression.clone();
                    IconButton::new(phosphor::X)
                        .size(Size::Xs)
                        .tooltip("Remove watch")
                        .on_click(move || remove(DebugCommand::RemoveWatch(watched.clone())))
                        .show();
                },
            );
        });
    }
    entry_box(
        "Add a watch expression",
        actions.debug_command.clone(),
        DebugCommand::AddWatch,
    );
}

/// The console's lines, kept scrolled to the newest unless the reader scrolled up.
fn console(lines: &[String]) {
    let list = remember_lazy_list_state();
    list.set_stick_to_end(true);
    lazy_list_keyed(
        Modifier::new().fill_max_width().weight(1.0).clip(),
        &list,
        lines.len(),
        |index| index,
        |index| {
            Label::new(lines[index].clone())
                .variant(LabelVariant::Code)
                .size(Size::Xs)
                .show();
        },
    );
}

/// A box that asks what the expression typed in it comes to.
fn evaluate_box(actions: &Actions) {
    entry_box(
        "Evaluate an expression",
        actions.debug_command.clone(),
        DebugCommand::Evaluate,
    );
}

/// A code box reading `placeholder` that hands what is typed in it, made into a command by `make`, to `send` on Enter, then empties.
fn entry_box(
    placeholder: &'static str,
    send: Rc<dyn Fn(DebugCommand)>,
    make: fn(String) -> DebugCommand,
) {
    let typed = remember(String::new);
    let change = typed.clone();
    let clear = typed.clone();
    TextInput::new(typed.get())
        .placeholder(placeholder)
        .size(Size::Xs)
        .code()
        .modifier(Modifier::new().fill_max_width())
        .on_change(move |text| change.set(text))
        .on_submit(move |text| {
            if !text.trim().is_empty() {
                send(make(text));
                clear.set(String::new());
            }
        })
        .show();
}

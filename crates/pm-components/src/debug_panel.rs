//! The debug tab of the dock: the controls over a program being debugged, its stack, its variables and watches, and its console.

use crate::areas::DebugEvent;
use crate::button::Variant;
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::sp;
use pm_protocol::{DebugCategory, DebugView, Standing};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, Color, column_arranged};
use rustle::{BoxAlignment, Modifier};

use crate::Button;
use crate::debug::{DEBUG_ROWS, Run, variable_rows};
use crate::icons::icon;
use crate::input::Input;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::theme::Role;

/// How wide the list of the stack is, in logical pixels.
const STACK_WIDTH: f32 = 220.0;

/// What a standing is called.
fn standing_words(view: &DebugView) -> String {
    match (view.standing, &view.reason) {
        (Standing::Starting, _) => "Starting".to_owned(),
        (Standing::Running, _) => "Running".to_owned(),
        (Standing::Stopped, Some(reason)) => format!("Paused: {reason}"),
        (Standing::Stopped, None) => "Paused".to_owned(),
        (Standing::Ended, _) => "Ended".to_owned(),
    }
}

/// Emits the controls over the program: run on or pause, the three steps and stop, with what the program is doing beside them.
#[track_caller]
fn controls(run: &Run) {
    let view = &run.view;
    let paused = view.standing == Standing::Stopped;
    let live = view.standing != Standing::Ended;
    let resume = if paused || !live {
        (
            phosphor::SKIP_FORWARD,
            DebugEvent::DebugContinue,
            "Continue",
            paused,
        )
    } else {
        let running = view.standing == Standing::Running;
        (phosphor::PAUSE, DebugEvent::DebugPause, "Pause", running)
    };
    kit::bar(9, 3, 1, || {
        for (icon, event, tip, enabled) in [
            resume,
            (
                phosphor::ARROW_ARC_RIGHT,
                DebugEvent::DebugStepOver,
                "Step over",
                paused,
            ),
            (
                phosphor::ARROW_LINE_DOWN,
                DebugEvent::DebugStepIn,
                "Step into",
                paused,
            ),
            (
                phosphor::ARROW_LINE_UP,
                DebugEvent::DebugStepOut,
                "Step out",
                paused,
            ),
            (phosphor::STOP, DebugEvent::DebugStop, "Stop", live),
        ] {
            rustle::key(tip, || {
                Button::icon(icon, Target::Debug(event))
                    .disabled(!enabled)
                    .tooltip(tip)
                    .build();
            });
        }
        for (said, role) in [
            (view.label.clone(), Role::Muted),
            (standing_words(view), Role::Subtle),
        ] {
            let own = Modifier::new().padding_each(theme::sp(2), 0.0, 0.0, 0.0);
            kit::text(own, said, kit::line(12, role));
        }
    });
}

/// One line of code-like text 12 pixels tall in `tone`, in the mono family with the reader's ligatures.
fn code_line(tone: impl Into<Color>) -> rustle::TextStyle {
    kit::line(12, tone)
        .font_family(theme::mono())
        .ligatures(theme::code_ligatures())
}

/// Emits `title` over a list of the tab, in a row `height` theme steps tall.
#[track_caller]
fn heading(title: &str, height: f32) {
    kit::row(
        Modifier::new()
            .height(theme::sp(height))
            .padding_xy(theme::sp(3), 0.0),
        0,
        || kit::text(Modifier::new(), title, kit::line(11, Role::Subtle)),
    );
}

/// Emits a column of the tab under `title`, holding what `body` emits, `width` logical pixels wide when it is given one and taking the room left otherwise.
#[track_caller]
fn section(title: &str, width: Option<f32>, body: impl FnOnce()) {
    let size = match width {
        Some(width) => Modifier::new().fill_max_height().width(width),
        None => Modifier::new().fill_max_height().weight(1.0),
    };
    kit::column(size.ruled(theme::Side::Left), 0, || {
        heading(title, 7.0);
        body();
    });
}

/// Emits a box scrolled as `scrolled` says, holding the rows `content` emits.
#[track_caller]
fn rows_box(scrolled: &rustle::ScrollState, content: impl FnOnce()) {
    kit::column(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .vertical_scroll(scrolled)
            .fill_max_width(),
        0,
        content,
    );
}

/// Emits one row of a list holding what `content` emits, pressed as `id` when it is given one, marked while `selected`.
#[track_caller]
fn list_row(id: Option<Target>, selected: bool, content: impl FnOnce()) {
    let own = Modifier::new().fill_max_width().height(theme::sp(6));
    let own = match id {
        Some(id) => {
            let looks = theme::InteractionTones::new().hover(Role::SurfaceHover);
            let looks = if selected {
                looks.idle(Role::SurfaceSelected)
            } else {
                looks
            };
            kit::pressed(own.indicated(looks, theme::Focused::Rustle).clip(), id)
        }
        None if selected => own.background(theme::color(Role::SurfaceSelected)).clip(),
        None => own.clip(),
    };
    kit::row(own.padding_xy(theme::sp(3), 0.0), 2, content);
}

/// Emits a row of a list that only says `said`, quietly.
#[track_caller]
fn note_row(said: &str) {
    list_row(None, false, || {
        kit::text(Modifier::new(), said, kit::line(12, Role::Subtle))
    });
}

/// Emits the stack of the paused thread, the frame being looked at marked.
#[track_caller]
fn stack(state: &WorkspaceState, run: &Run) {
    let view = &run.view;
    section("Call stack", Some(STACK_WIDTH), || {
        rows_box(&state.debug.stack_scroll, || {
            if view.frames.is_empty() {
                note_row(if view.standing == Standing::Running {
                    "Running"
                } else {
                    "Not paused"
                });
                return;
            }
            for (index, frame) in view.frames.iter().take(DEBUG_ROWS).enumerate() {
                let place = frame
                    .path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| format!("{}:{}", name.to_string_lossy(), frame.line + 1))
                    .unwrap_or_default();
                rustle::key(index, || {
                    list_row(
                        Some(Target::Debug(DebugEvent::DebugFrame(index))),
                        view.frame == Some(frame.id),
                        || {
                            kit::text(
                                Modifier::new(),
                                frame.name.clone(),
                                code_line(Role::Text).max_lines(1),
                            );
                            kit::text(Modifier::new(), place, kit::line(12, Role::Subtle));
                        },
                    );
                });
            }
        });
    });
}

/// Emits one variable or scope, the row at `index`, indented by its depth, with a chevron when it has members.
fn variable_row(index: usize, entry: &crate::debug::VariableRow) {
    let id = (entry.reference != 0).then_some(Target::Debug(DebugEvent::DebugVariable(index)));
    let entry = entry.clone();
    list_row(id, false, move || {
        rustle::with_constraints(Modifier::new().fill_max_width(), move |room| {
            variable_parts(&entry, room.max_width);
        });
    });
}

/// How wide, in logical pixels, the chevron of a variable with members is.
const VARIABLE_CHEVRON: f32 = 12.0;

/// How wide `words` are set in `style`, in whole logical pixels.
fn width_of(words: &str, style: &rustle::TextStyle) -> f32 {
    rustle::layout_text(words, style, None).size().width.ceil()
}

/// Emits the parts of the row of `entry` in `room` logical pixels across: its indent, its chevron or the room of one, its name, and, when it holds one, `=` and its value, then its type.
///
/// The value takes the room the others leave. When there is none, the chevron and the type give up the overflow in proportion to their widths, the chevron drawn smaller and the type cutting itself short, and the value has no room.
fn variable_parts(entry: &crate::debug::VariableRow, room: f32) {
    let tone = if entry.value.is_empty() {
        Role::Muted
    } else {
        Role::Text
    };
    let name_kind = code_line(tone);
    let equals_kind = kit::line(12, Role::Subtle);
    let type_kind = kit::line(11, Role::Subtle);
    let indent = entry.depth as f32 * 12.0;
    let typed = entry
        .kind
        .as_deref()
        .map_or(0.0, |kind| width_of(kind, &type_kind));
    let mut used = indent + VARIABLE_CHEVRON + width_of(&entry.name, &name_kind) + typed;
    let mut parts = 3 + usize::from(entry.kind.is_some());
    if !entry.value.is_empty() {
        used += width_of("=", &equals_kind);
        parts += 2;
    }
    used += sp(2) * (parts - 1) as f32;
    let over = (used - room).max(0.0);
    let giving = if entry.reference != 0 {
        VARIABLE_CHEVRON
    } else {
        0.0
    } + typed;
    let kept = if giving > 0.0 {
        (1.0 - over / giving).clamp(0.0, 1.0)
    } else {
        1.0
    };
    kit::row(Modifier::new().fill_max_width(), 2, || {
        rustle::spacer(Modifier::new().width(indent));
        if entry.reference != 0 {
            rustle::stack_aligned(
                Modifier::new().size(VARIABLE_CHEVRON * kept, VARIABLE_CHEVRON),
                BoxAlignment::Center,
                || icon(kit::chevron(entry.unfolded), 3.0 * kept, Role::Muted),
            );
        } else {
            rustle::spacer(Modifier::new().width(VARIABLE_CHEVRON));
        }
        kit::text(Modifier::new(), entry.name.clone(), name_kind);
        if !entry.value.is_empty() {
            kit::text(Modifier::new(), "=", equals_kind);
            let value_kind = code_line(Role::SyntaxString);
            kit::text(Modifier::new().weight(1.0), entry.value.clone(), value_kind);
        }
        if let Some(kind) = &entry.kind {
            kit::text(
                Modifier::new().width_in(0.0, typed * kept),
                kind.clone(),
                type_kind,
            );
        }
    });
}

/// Emits the small field `id` of the tab over `held`, saying `placeholder` while empty, padded as the tab's fields are.
#[track_caller]
fn field(held: &rustle::TextFieldState, id: Target, placeholder: &str) {
    kit::column(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(2), theme::sp(1)),
        0,
        || {
            Input::new(held, id)
                .size(Size::Sm)
                .placeholder(placeholder)
                .build();
        },
    );
}

/// Emits the variables of the frame being looked at over the expressions watched and the field that adds one.
#[track_caller]
fn variables(state: &WorkspaceState, run: &Run) {
    let view = &run.view;
    let rows = variable_rows(view, &state.debug.unfolded);
    section("Variables", None, || {
        rows_box(&state.debug.variables_scroll, || {
            if rows.is_empty() {
                note_row("Nothing to show");
            }
            for (index, entry) in rows.iter().take(DEBUG_ROWS).enumerate() {
                rustle::key(index, || variable_row(index, entry));
            }
            heading("Watch", 6.0);
            for (index, expression) in state.debug.watches.iter().take(DEBUG_ROWS).enumerate() {
                let shown = match view
                    .watched
                    .iter()
                    .find(|held| &held.expression == expression)
                {
                    Some(held) => match &held.value {
                        Ok(value) => (value.value.clone(), Role::SyntaxString),
                        Err(reason) => (reason.clone(), Role::Danger),
                    },
                    None => (String::new(), Role::Subtle),
                };
                rustle::key(("watch", index), || {
                    list_row(None, false, || {
                        kit::text(Modifier::new(), expression.clone(), code_line(Role::Text));
                        kit::text(Modifier::new(), "=", kit::line(12, Role::Subtle));
                        kit::text(Modifier::new().weight(1.0), shown.0, code_line(shown.1));
                        Button::icon(
                            phosphor::X,
                            Target::Debug(DebugEvent::DebugWatchRemove(index)),
                        )
                        .build();
                    });
                });
            }
            field(
                &state.debug.watch,
                Target::Debug(DebugEvent::DebugWatchField),
                "Add an expression to watch",
            );
        });
    });
}

/// What a console line of `category` is drawn in.
fn console_tone(category: DebugCategory) -> Role {
    match category {
        DebugCategory::Console => Role::Muted,
        DebugCategory::Stdout => Role::Text,
        DebugCategory::Stderr | DebugCategory::Error => Role::Danger,
        DebugCategory::Asked => Role::Subtle,
        DebugCategory::Answer => Role::SyntaxString,
    }
}

/// Emits the console: what the program and the adapter printed, scrolled back from its end, and the field that asks the program what an expression comes to.
#[track_caller]
fn console(state: &WorkspaceState, run: &Run) {
    section("Console", None, || {
        kit::column(Modifier::new().fill_max_width().weight(1.0), 0, || {
            kit::column(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .vertical_scroll_with(&state.debug.console_back, true)
                    .fill_max_width()
                    .padding_xy(theme::sp(3), 0.0),
                0,
                || {
                    for entry in &run.view.lines {
                        kit::text(
                            Modifier::new(),
                            entry.text.clone(),
                            theme::code_style(12, 1.4)
                                .color(theme::color(console_tone(entry.category)))
                                .max_lines(1),
                        );
                    }
                },
            );
            field(
                &state.debug.console,
                Target::Debug(DebugEvent::DebugConsoleField),
                "Evaluate an expression",
            );
        });
    });
}

/// Emits what the tab holds while nothing is being debugged: the way to start.
#[track_caller]
fn idle(state: &WorkspaceState) {
    let said = state
        .debug
        .trouble
        .clone()
        .unwrap_or_else(|| "Nothing is being debugged.".to_owned());
    column_arranged(
        Modifier::new().weight(1.0).padding_xy(0.0, theme::sp(6)),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        || {
            kit::text(Modifier::new(), said, kit::line(13, Role::Subtle));
            kit::action(
                "Start debugging",
                Target::Debug(DebugEvent::DebugStart),
                Variant::Default,
            );
        },
    );
}

/// Emits the body of the debug tab.
pub fn debug_tab(state: &WorkspaceState) {
    match &state.debug.run {
        Some(run) => kit::column(Modifier::new().fill_max_width().weight(1.0), 0, || {
            controls(run);
            kit::row_top(Modifier::new().fill_max_width().weight(1.0), 0, || {
                stack(state, run);
                variables(state, run);
                console(state, run);
            });
        }),
        None => idle(state),
    }
}

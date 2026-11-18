//! The debug tab of the dock: the controls over a program being debugged, its stack, its variables and watches, and its console.

use pm_protocol::{DebugCategory, DebugView, Standing};
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::debug::{
    DEBUG_CONSOLE_FIELD, DEBUG_CONSOLE_SCROLL, DEBUG_CONTINUE, DEBUG_FRAME_BASE, DEBUG_PAUSE,
    DEBUG_ROWS, DEBUG_STACK_SCROLL, DEBUG_START, DEBUG_STEP_IN, DEBUG_STEP_OUT, DEBUG_STEP_OVER,
    DEBUG_STOP, DEBUG_VARIABLE_BASE, DEBUG_VARIABLES_SCROLL, DEBUG_WATCH_FIELD,
    DEBUG_WATCH_REMOVE_BASE, Run, variable_rows,
};
use crate::input::Input;
use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Scroll, Side, Styled, Tone,
    Tooltip, Type, block, column, gap_box, icon, label, label_with, row, scroll_area,
};
use crate::size::Size;
use crate::state::WorkspaceState;

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

/// The type of a line of the tab: `pixels` tall with a line height of 1.4 times that, in `tone`, cut short at the edge of its box.
fn small(pixels: u32, tone: impl Into<Tone>) -> Type {
    Type::new(pixels).ratio(1.4).tone(tone).truncate()
}

/// Emits one icon button of the controls: `icon` answering to `id`, explained by `tip`, pressable while `enabled`.
#[track_caller]
fn control(icon: IconName, id: FocusId, tip: &'static str, enabled: bool) {
    Button::icon(icon, id)
        .size(Size::Sm)
        .disabled(!enabled)
        .modifier(Modifier::new().tooltip(tip))
        .build();
}

/// Emits the controls over the program: run on or pause, the three steps and stop, with what the program is doing beside them.
#[track_caller]
fn controls(run: &Run) {
    let view = &run.view;
    let paused = view.standing == Standing::Stopped;
    let live = view.standing != Standing::Ended;
    row(
        Modifier::new()
            .h(9)
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .pad_x(3),
        Flex::new().center().gap(1),
        || {
            if paused || !live {
                control(IconName::Continue, DEBUG_CONTINUE, "Continue", paused);
            } else {
                control(
                    IconName::Pause,
                    DEBUG_PAUSE,
                    "Pause",
                    view.standing == Standing::Running,
                );
            }
            control(IconName::StepOver, DEBUG_STEP_OVER, "Step over", paused);
            control(IconName::StepInto, DEBUG_STEP_IN, "Step into", paused);
            control(IconName::StepOut, DEBUG_STEP_OUT, "Step out", paused);
            control(IconName::Stop, DEBUG_STOP, "Stop", live);
            label_with(
                Modifier::new().pad_side(Side::Left, 2),
                view.label.clone(),
                small(12, Role::Muted),
            );
            label_with(
                Modifier::new().pad_side(Side::Left, 2),
                standing_words(view),
                small(12, Role::Subtle),
            );
        },
    );
}

/// Emits a column of the tab under `title`, holding what `body` emits, `width` logical pixels wide when it is given one and taking the room left otherwise.
#[track_caller]
fn section(title: &str, width: Option<f32>, body: impl FnOnce()) {
    let size = match width {
        Some(width) => Modifier::new().width(width),
        None => Modifier::new().grow(),
    };
    column(
        size.stroke_side(Side::Left, 1.0, Role::Border),
        Flex::new(),
        || {
            row(Modifier::new().h(7).pad_x(3), Flex::new().center(), || {
                label(title, small(11, Role::Subtle));
            });
            body();
        },
    );
}

/// Emits a box scrolled `scrolled` pixels from the top by the list `id`, holding the rows `content` emits.
#[track_caller]
fn rows_box(id: FocusId, scrolled: f32, content: impl FnOnce()) {
    scroll_area(
        Modifier::new().grow(),
        Scroll::from_start(id, scrolled),
        || column(Modifier::new().fill_max_width(), Flex::new(), content),
    );
}

/// Emits one row of a list holding what `content` emits, pressed as `id` when it is given one, marked while `selected`.
#[track_caller]
fn list_row(id: Option<FocusId>, selected: bool, content: impl FnOnce()) {
    let mut own = Modifier::new().h(6);
    own = match id {
        Some(id) => {
            let looks = Looks::new().hover(Role::SurfaceHover);
            let looks = if selected {
                looks.idle(Role::SurfaceSelected)
            } else {
                looks
            };
            own.looks(looks)
                .clip_to(Radius::None)
                .cursor(CursorIcon::Pointer)
                .target(id)
        }
        None if selected => own.fill(Role::SurfaceSelected).clip_to(Radius::None),
        None => own.clip_to(Radius::None),
    };
    row(own.pad_x(3), Flex::new().center().gap(2), content);
}

/// Emits the stack of the paused thread, the frame being looked at marked.
#[track_caller]
fn stack(state: &WorkspaceState, run: &Run) {
    let view = &run.view;
    section("Call stack", Some(STACK_WIDTH), || {
        rows_box(DEBUG_STACK_SCROLL, state.debug.stack_scroll, || {
            if view.frames.is_empty() {
                let said = if view.standing == Standing::Running {
                    "Running"
                } else {
                    "Not paused"
                };
                list_row(None, false, || label(said, small(12, Role::Subtle)));
                return;
            }
            for (index, frame) in view.frames.iter().take(DEBUG_ROWS as usize).enumerate() {
                let place = frame
                    .path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| format!("{}:{}", name.to_string_lossy(), frame.line + 1))
                    .unwrap_or_default();
                rustle::key(index, || {
                    list_row(
                        Some(FocusId(DEBUG_FRAME_BASE + index as u64)),
                        view.frame == Some(frame.id),
                        || {
                            label_with(
                                Modifier::new().grow(),
                                frame.name.clone(),
                                small(12, Role::Text).mono(),
                            );
                            label(place, small(12, Role::Subtle));
                        },
                    );
                });
            }
        });
    });
}

/// Emits one variable or scope, the row at `index`, indented by its depth, with a chevron when it has members.
#[track_caller]
fn variable_row(index: usize, entry: &crate::debug::VariableRow) {
    let id = (entry.reference != 0).then_some(FocusId(DEBUG_VARIABLE_BASE + index as u64));
    list_row(id, false, || {
        gap_box(Modifier::new().width(entry.depth as f32 * 12.0));
        if entry.reference != 0 {
            icon(
                if entry.unfolded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                },
                3.0,
                Role::Muted,
            );
        } else {
            gap_box(Modifier::new().width(12.0));
        }
        let tone: Tone = if entry.value.is_empty() {
            Role::Muted.into()
        } else {
            Role::Text.into()
        };
        label(entry.name.clone(), small(12, tone).mono());
        if !entry.value.is_empty() {
            label("=", small(12, Role::Subtle));
            label_with(
                Modifier::new().grow(),
                entry.value.clone(),
                small(12, Role::SyntaxString).mono(),
            );
        }
        if let Some(kind) = &entry.kind {
            label(kind.clone(), small(11, Role::Subtle));
        }
    });
}

/// Emits a field, padded as the tab's fields are, from what `field` emits.
#[track_caller]
fn field_box(field: impl FnOnce()) {
    block(Modifier::new().pad_x(2).pad_y(1), field);
}

/// Emits the variables of the frame being looked at over the expressions watched and the field that adds one.
#[track_caller]
fn variables(state: &WorkspaceState, run: &Run) {
    let view = &run.view;
    let rows = variable_rows(view, &state.debug.unfolded);
    section("Variables", None, || {
        rows_box(DEBUG_VARIABLES_SCROLL, state.debug.variables_scroll, || {
            if rows.is_empty() {
                list_row(None, false, || {
                    label("Nothing to show", small(12, Role::Subtle))
                });
            }
            for (index, entry) in rows.iter().take(DEBUG_ROWS as usize).enumerate() {
                rustle::key(index, || variable_row(index, entry));
            }
            row(Modifier::new().h(6).pad_x(3), Flex::new().center(), || {
                label("Watch", small(11, Role::Subtle));
            });
            for (index, expression) in state
                .debug
                .watches
                .iter()
                .take(DEBUG_ROWS as usize)
                .enumerate()
            {
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
                        label(expression.clone(), small(12, Role::Text).mono());
                        label("=", small(12, Role::Subtle));
                        label_with(Modifier::new().grow(), shown.0, small(12, shown.1).mono());
                        Button::icon(
                            IconName::Close,
                            FocusId(DEBUG_WATCH_REMOVE_BASE + index as u64),
                        )
                        .size(Size::Sm)
                        .build();
                    });
                });
            }
            field_box(|| {
                Input::new(&state.debug.watch, DEBUG_WATCH_FIELD)
                    .size(Size::Sm)
                    .placeholder("Add an expression to watch")
                    .build();
            });
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
        column(Modifier::new().grow(), Flex::new(), || {
            scroll_area(
                Modifier::new().grow(),
                Scroll::from_end(DEBUG_CONSOLE_SCROLL, state.debug.console_back),
                || {
                    column(
                        Modifier::new().fill_max_width().pad_x(3),
                        Flex::new(),
                        || {
                            for entry in &run.view.lines {
                                label(
                                    entry.text.clone(),
                                    Type::code(12)
                                        .ratio(1.4)
                                        .tone(console_tone(entry.category))
                                        .truncate(),
                                );
                            }
                        },
                    );
                },
            );
            field_box(|| {
                Input::new(&state.debug.console, DEBUG_CONSOLE_FIELD)
                    .size(Size::Sm)
                    .placeholder("Evaluate an expression")
                    .build();
            });
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
    column(
        Modifier::new().grow().pad_y(6),
        Flex::new().center().gap(3),
        || {
            label(said, small(13, Role::Subtle));
            Button::new("Start debugging", DEBUG_START)
                .size(Size::Sm)
                .build();
        },
    );
}

/// Emits the body of the debug tab.
#[track_caller]
pub fn debug_tab(state: &WorkspaceState) {
    match &state.debug.run {
        Some(run) => column(Modifier::new().grow(), Flex::new(), || {
            controls(run);
            row(Modifier::new().grow(), Flex::new(), || {
                stack(state, run);
                variables(state, run);
                console(state, run);
            });
        }),
        None => idle(state),
    }
}

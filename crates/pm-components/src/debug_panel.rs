//! The debug tab of the dock: the controls over a program being debugged, its stack, its variables and watches, and its console.

use crate::events::reduce;
use crate::theme;
use crate::theme::sp;
use pm_protocol::{DebugCategory, DebugView, Standing};
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{BoxAlignment, CursorIcon, Modifier};

use crate::Button;
use crate::debug::{
    DEBUG_CONSOLE_FIELD, DEBUG_CONTINUE, DEBUG_FRAME_BASE, DEBUG_PAUSE, DEBUG_ROWS, DEBUG_START,
    DEBUG_STEP_IN, DEBUG_STEP_OUT, DEBUG_STEP_OVER, DEBUG_STOP, DEBUG_VARIABLE_BASE,
    DEBUG_WATCH_FIELD, DEBUG_WATCH_REMOVE_BASE, Run, variable_rows,
};
use crate::events::Interactive;
use crate::focus::FocusId;
use crate::icons::IconName;
use crate::icons::icon;
use crate::input::Input;
use crate::select::styled_text;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::theme::{Role, Tone};

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
fn small(pixels: u32, tone: impl Into<Tone>) -> rustle::TextStyle {
    theme::text_style(pixels, 1.4)
        .color(theme::color(tone))
        .max_lines(1)
}

/// Emits one icon button of the controls: `icon` answering to `id`, explained by `tip`, pressable while `enabled`.
#[track_caller]
fn control(icon: IconName, id: FocusId, tip: &'static str, enabled: bool) {
    Button::icon(icon, id)
        .size(Size::Sm)
        .disabled(!enabled)
        .tooltip(tip)
        .build();
}

/// Emits the controls over the program: run on or pause, the three steps and stop, with what the program is doing beside them.
#[track_caller]
fn controls(run: &Run) {
    let view = &run.view;
    let paused = view.standing == Standing::Stopped;
    let live = view.standing != Standing::Ended;
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(9))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Center,
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
            styled_text(
                Modifier::new().padding_each(theme::sp(2), 0.0, 0.0, 0.0),
                view.label.clone(),
                small(12, Role::Muted),
            );
            styled_text(
                Modifier::new().padding_each(theme::sp(2), 0.0, 0.0, 0.0),
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
        Some(width) => Modifier::new().fill_max_height().width(width),
        None => Modifier::new().fill_max_height().weight(1.0),
    };
    column_arranged(
        size.draw_behind(theme::edge(
            theme::Side::Left,
            1.0,
            theme::color(Role::Border),
        ))
        .padding_each(1.0, 0.0, 0.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .height(theme::sp(7))
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::Start,
                Alignment::Center,
                || {
                    styled_text(Modifier::new(), title, small(11, Role::Subtle));
                },
            );
            body();
        },
    );
}

/// Emits a box scrolled as `scrolled` says, holding the rows `content` emits.
#[track_caller]
fn rows_box(scrolled: &rustle::ScrollState, content: impl FnOnce()) {
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .vertical_scroll(scrolled)
            .fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        content,
    );
}

/// Emits one row of a list holding what `content` emits, pressed as `id` when it is given one, marked while `selected`.
#[track_caller]
fn list_row(id: Option<FocusId>, selected: bool, content: impl FnOnce()) {
    let mut own = Modifier::new().fill_max_width().height(theme::sp(6));
    own = match id {
        Some(id) => {
            let looks = theme::InteractionTones::new().hover(Role::SurfaceHover);
            let looks = if selected {
                looks.idle(Role::SurfaceSelected)
            } else {
                looks
            };
            own.interaction_background(looks.colors())
                .clip()
                .cursor(CursorIcon::Pointer)
                .control(id, reduce(id))
        }
        None if selected => own.background(theme::color(Role::SurfaceSelected)).clip(),
        None => own.clip(),
    };
    row_arranged(
        own.padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        content,
    );
}

/// Emits the stack of the paused thread, the frame being looked at marked.
#[track_caller]
fn stack(state: &WorkspaceState, run: &Run) {
    let view = &run.view;
    section("Call stack", Some(STACK_WIDTH), || {
        rows_box(&state.debug.stack_scroll, || {
            if view.frames.is_empty() {
                let said = if view.standing == Standing::Running {
                    "Running"
                } else {
                    "Not paused"
                };
                list_row(None, false, || {
                    styled_text(Modifier::new(), said, small(12, Role::Subtle))
                });
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
                            styled_text(
                                Modifier::new(),
                                frame.name.clone(),
                                small(12, Role::Text)
                                    .font_family(theme::mono())
                                    .ligatures(theme::code_ligatures())
                                    .max_lines(1),
                            );
                            styled_text(Modifier::new(), place, small(12, Role::Subtle));
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
    let entry = entry.clone();
    list_row(id, false, move || {
        rustle::with_constraints(Modifier::new().fill_max_width(), move |room| {
            variable_parts(&entry, room.max_width);
        });
    });
}

/// How wide, in logical pixels, the chevron of a variable with members is.
const VARIABLE_CHEVRON: f32 = 12.0;

/// Emits the parts of the row of `entry` in `room` logical pixels across: its indent, its chevron or the room of one, its name, and, when it holds one, `=` and its value, then its type.
///
/// The value takes the room the others leave. When there is none, the chevron and the type give up the overflow in proportion to their widths, the chevron drawn smaller and the type cutting itself short, and the value has no room.
#[track_caller]
fn variable_parts(entry: &crate::debug::VariableRow, room: f32) {
    let tone: Tone = if entry.value.is_empty() {
        Role::Muted.into()
    } else {
        Role::Text.into()
    };
    let name_kind = small(12, tone)
        .font_family(theme::mono())
        .ligatures(theme::code_ligatures());
    let equals_kind = small(12, Role::Subtle);
    let value_kind = small(12, Role::SyntaxString)
        .font_family(theme::mono())
        .ligatures(theme::code_ligatures());
    let type_kind = small(11, Role::Subtle);
    let indent = entry.depth as f32 * 12.0;
    let typed = entry.kind.as_deref().map_or(0.0, |kind| {
        rustle::layout_text(kind, &type_kind, None)
            .size()
            .width
            .ceil()
    });
    let mut used = indent
        + VARIABLE_CHEVRON
        + rustle::layout_text(&entry.name, &name_kind, None)
            .size()
            .width
            .ceil()
        + typed;
    let mut parts = 3 + usize::from(entry.kind.is_some());
    if !entry.value.is_empty() {
        used += rustle::layout_text("=", &equals_kind, None)
            .size()
            .width
            .ceil();
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
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            rustle::spacer(Modifier::new().width(indent));
            if entry.reference != 0 {
                rustle::stack_aligned(
                    Modifier::new().size(VARIABLE_CHEVRON * kept, VARIABLE_CHEVRON),
                    BoxAlignment::Center,
                    || {
                        icon(
                            if entry.unfolded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            },
                            3.0 * kept,
                            Role::Muted,
                        );
                    },
                );
            } else {
                rustle::spacer(Modifier::new().width(VARIABLE_CHEVRON));
            }
            styled_text(Modifier::new(), entry.name.clone(), name_kind);
            if !entry.value.is_empty() {
                styled_text(Modifier::new(), "=", equals_kind);
                styled_text(Modifier::new().weight(1.0), entry.value.clone(), value_kind);
            }
            if let Some(kind) = &entry.kind {
                styled_text(
                    Modifier::new().width_in(0.0, typed * kept),
                    kind.clone(),
                    type_kind,
                );
            }
        },
    );
}

/// Emits a field, padded as the tab's fields are, from what `field` emits.
#[track_caller]
fn field_box(field: impl FnOnce()) {
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(2), 0.0)
            .padding_xy(0.0, theme::sp(1)),
        Arrangement::Start,
        Alignment::Start,
        field,
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
                list_row(None, false, || {
                    styled_text(Modifier::new(), "Nothing to show", small(12, Role::Subtle))
                });
            }
            for (index, entry) in rows.iter().take(DEBUG_ROWS as usize).enumerate() {
                rustle::key(index, || variable_row(index, entry));
            }
            row_arranged(
                Modifier::new()
                    .height(theme::sp(6))
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::Start,
                Alignment::Center,
                || {
                    styled_text(Modifier::new(), "Watch", small(11, Role::Subtle));
                },
            );
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
                        styled_text(
                            Modifier::new(),
                            expression.clone(),
                            small(12, Role::Text)
                                .font_family(theme::mono())
                                .ligatures(theme::code_ligatures()),
                        );
                        styled_text(Modifier::new(), "=", small(12, Role::Subtle));
                        styled_text(
                            Modifier::new().weight(1.0),
                            shown.0,
                            small(12, shown.1)
                                .font_family(theme::mono())
                                .ligatures(theme::code_ligatures()),
                        );
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
        column_arranged(
            Modifier::new().fill_max_width().weight(1.0),
            Arrangement::Start,
            Alignment::Start,
            || {
                column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .weight(1.0)
                        .vertical_scroll_with(&state.debug.console_back, true)
                        .fill_max_width()
                        .padding_xy(theme::sp(3), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        for entry in &run.view.lines {
                            styled_text(
                                Modifier::new(),
                                entry.text.clone(),
                                theme::code_style(12, 1.4)
                                    .color(theme::color(console_tone(entry.category)))
                                    .max_lines(1),
                            );
                        }
                    },
                );
                field_box(|| {
                    Input::new(&state.debug.console, DEBUG_CONSOLE_FIELD)
                        .size(Size::Sm)
                        .placeholder("Evaluate an expression")
                        .build();
                });
            },
        );
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
            styled_text(Modifier::new(), said, small(13, Role::Subtle));
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
        Some(run) => column_arranged(
            Modifier::new().fill_max_width().weight(1.0),
            Arrangement::Start,
            Alignment::Start,
            || {
                controls(run);
                row_arranged(
                    Modifier::new().fill_max_width().weight(1.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        stack(state, run);
                        variables(state, run);
                        console(state, run);
                    },
                );
            },
        ),
        None => idle(state),
    }
}

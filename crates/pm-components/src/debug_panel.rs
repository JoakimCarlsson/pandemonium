//! The debug tab of the dock: the controls over a program being debugged, its stack, its variables and watches, and its console.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_protocol::{DebugCategory, DebugView, Standing};
use pm_style::{
    Color, Role, bg, border_b, border_color, border_l, cursor_pointer, flex, flex_1, flex_col,
    flex_none, font_mono, gap, h, items_center, min_h, min_w_px, overflow_hidden, pl, px, py,
    text_color, w_px,
};
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::Size;
use crate::debug::{
    DEBUG_CONSOLE_FIELD, DEBUG_CONSOLE_SCROLL, DEBUG_CONTINUE, DEBUG_FRAME_BASE, DEBUG_PAUSE,
    DEBUG_ROWS, DEBUG_STACK_SCROLL, DEBUG_START, DEBUG_STEP_IN, DEBUG_STEP_OUT, DEBUG_STEP_OVER,
    DEBUG_STOP, DEBUG_VARIABLE_BASE, DEBUG_VARIABLES_SCROLL, DEBUG_WATCH_FIELD,
    DEBUG_WATCH_REMOVE_BASE, Run, variable_rows,
};
use crate::input::{Input, Size as InputSize};
use crate::parts::{glyph, line, spacer};
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

/// The controls over the program: run on or pause, the three steps and stop, with what the program is doing beside them.
fn controls(run: &Run) -> Node {
    let view = &run.view;
    let paused = view.standing == Standing::Stopped;
    let live = view.standing != Standing::Ended;
    let button = |icon: IconName, id: FocusId, tip: &'static str, enabled: bool| {
        Button::icon(icon, id)
            .size(Size::Icon)
            .disabled(!enabled)
            .build()
            .tooltip(tip)
    };
    let run_on = if paused || !live {
        button(IconName::Continue, DEBUG_CONTINUE, "Continue", paused)
    } else {
        button(
            IconName::Pause,
            DEBUG_PAUSE,
            "Pause",
            view.standing == Standing::Running,
        )
    };
    node()
        .styles([
            flex(),
            items_center(),
            gap(1),
            h(9),
            px(3),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(run_on)
        .child(button(
            IconName::StepOver,
            DEBUG_STEP_OVER,
            "Step over",
            paused,
        ))
        .child(button(
            IconName::StepInto,
            DEBUG_STEP_IN,
            "Step into",
            paused,
        ))
        .child(button(
            IconName::StepOut,
            DEBUG_STEP_OUT,
            "Step out",
            paused,
        ))
        .child(button(IconName::Stop, DEBUG_STOP, "Stop", live))
        .child(line(view.label.clone(), 12, 1.4, Role::Muted).styles([pl(2), flex_none()]))
        .child(line(standing_words(view), 12, 1.4, Role::Subtle).styles([pl(2), flex_none()]))
}

/// A column of the tab under `title`, holding `body`, `width` logical pixels wide when it is given one.
fn column(title: &str, width: Option<f32>, body: Node) -> Node {
    let mut column = node()
        .styles([
            flex(),
            flex_col(),
            min_w_px(0),
            min_h(0),
            border_l(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), h(7), px(3), flex_none()])
                .child(line(title, 11, 1.4, Role::Subtle)),
        )
        .child(body);
    column = match width {
        Some(width) => column.styles([w_px(width), flex_none()]),
        None => column.styles([flex_1()]),
    };
    column
}

/// A box that holds rows and scrolls them from the top.
fn rows_box(id: FocusId, scrolled: f32) -> Node {
    node()
        .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
        .scroll_from_start(id, scrolled)
}

/// One row of a list: `content`, pressed as `id` when it is given one.
fn row(content: Vec<Node>, id: Option<FocusId>, selected: bool) -> Node {
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(6),
            px(3),
            flex_none(),
            overflow_hidden(),
        ])
        .children(content);
    if selected {
        row = row.styles([bg(Role::SurfaceSelected)]);
    }
    match id {
        Some(id) => row
            .styles([cursor_pointer(), bg(Role::SurfaceHover).hover()])
            .focusable(id),
        None => row,
    }
}

/// The stack of the paused thread, the frame being looked at marked.
fn stack(state: &WorkspaceState, run: &Run) -> Node {
    let view = &run.view;
    let mut body = rows_box(DEBUG_STACK_SCROLL, state.debug.stack_scroll);
    if view.frames.is_empty() {
        let said = if view.standing == Standing::Running {
            "Running"
        } else {
            "Not paused"
        };
        return column(
            "Call stack",
            Some(STACK_WIDTH),
            body.child(row(vec![line(said, 12, 1.4, Role::Subtle)], None, false)),
        );
    }
    for (index, frame) in view.frames.iter().take(DEBUG_ROWS as usize).enumerate() {
        let place = frame
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| format!("{}:{}", name.to_string_lossy(), frame.line + 1))
            .unwrap_or_default();
        body = body.child(row(
            vec![
                line(frame.name.clone(), 12, 1.4, Role::Text).styles([font_mono(), flex_none()]),
                line(place, 12, 1.4, Role::Subtle).styles([flex_none()]),
            ],
            Some(FocusId(DEBUG_FRAME_BASE + index as u64)),
            view.frame == Some(frame.id),
        ));
    }
    column("Call stack", Some(STACK_WIDTH), body)
}

/// One variable or scope, indented by its depth, with a chevron when it has members.
fn variable_row(index: usize, entry: &crate::debug::VariableRow) -> Node {
    let mut content = vec![node().styles([w_px(entry.depth as f32 * 12.0), flex_none()])];
    content.push(if entry.reference != 0 {
        glyph(
            if entry.unfolded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            },
            3.0,
            Role::Muted,
        )
    } else {
        node().styles([w_px(12.0), flex_none()])
    });
    let tone: Color = if entry.value.is_empty() {
        Role::Muted.into()
    } else {
        Role::Text.into()
    };
    content.push(line(entry.name.clone(), 12, 1.4, tone).styles([font_mono(), flex_none()]));
    if !entry.value.is_empty() {
        content.push(line("=", 12, 1.4, Role::Subtle).styles([flex_none()]));
        content.push(
            line(entry.value.clone(), 12, 1.4, Role::SyntaxString)
                .styles([font_mono(), flex_none()]),
        );
    }
    if let Some(kind) = &entry.kind {
        content.push(line(kind.clone(), 11, 1.4, Role::Subtle).styles([flex_none()]));
    }
    row(
        content,
        (entry.reference != 0).then_some(FocusId(DEBUG_VARIABLE_BASE + index as u64)),
        false,
    )
}

/// The variables of the frame being looked at over the expressions watched and the field that adds one.
fn variables(state: &WorkspaceState, run: &Run) -> Node {
    let view = &run.view;
    let mut body = rows_box(DEBUG_VARIABLES_SCROLL, state.debug.variables_scroll);
    let rows = variable_rows(view, &state.debug.unfolded);
    if rows.is_empty() {
        body = body.child(row(
            vec![line("Nothing to show", 12, 1.4, Role::Subtle)],
            None,
            false,
        ));
    }
    for (index, entry) in rows.iter().take(DEBUG_ROWS as usize).enumerate() {
        body = body.child(variable_row(index, entry));
    }
    body = body.child(
        node()
            .styles([h(6), px(3), flex(), items_center(), flex_none()])
            .child(line("Watch", 11, 1.4, Role::Subtle)),
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
        body = body.child(row(
            vec![
                line(expression.clone(), 12, 1.4, Role::Text).styles([font_mono(), flex_none()]),
                line("=", 12, 1.4, Role::Subtle).styles([flex_none()]),
                line(shown.0, 12, 1.4, shown.1).styles([font_mono(), flex_none()]),
                spacer(),
                Button::icon(
                    IconName::Close,
                    FocusId(DEBUG_WATCH_REMOVE_BASE + index as u64),
                )
                .size(Size::Icon)
                .build(),
            ],
            None,
            false,
        ));
    }
    let add = node().styles([px(2), py(1), flex_none()]).child(
        Input::new(&state.debug.watch, DEBUG_WATCH_FIELD)
            .size(InputSize::Sm)
            .placeholder("Add an expression to watch")
            .build(),
    );
    column("Variables", None, body.child(add))
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

/// The console: what the program and the adapter printed, scrolled back from its end, and the field that asks the program what an expression comes to.
fn console(state: &WorkspaceState, run: &Run) -> Node {
    let mut body = node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_h(0),
            overflow_hidden(),
            px(3),
        ])
        .scroll_from_end(DEBUG_CONSOLE_SCROLL, state.debug.console_back);
    for entry in &run.view.lines {
        body = body.child(text(entry.text.clone()).styles([
            pm_style::code_px(12),
            pm_style::leading_ratio(1.4),
            font_mono(),
            text_color(console_tone(entry.category)),
            flex_none(),
        ]));
    }
    let ask = node().styles([px(2), py(1), flex_none()]).child(
        Input::new(&state.debug.console, DEBUG_CONSOLE_FIELD)
            .size(InputSize::Sm)
            .placeholder("Evaluate an expression")
            .build(),
    );
    column(
        "Console",
        None,
        node()
            .styles([flex(), flex_col(), flex_1(), min_h(0)])
            .child(body)
            .child(ask),
    )
}

/// What the tab holds while nothing is being debugged: the way to start.
fn idle(state: &WorkspaceState) -> Node {
    let said = state
        .debug
        .trouble
        .clone()
        .unwrap_or_else(|| "Nothing is being debugged.".to_owned());
    node()
        .styles([flex(), flex_col(), items_center(), gap(3), py(6), flex_1()])
        .child(line(said, 13, 1.4, Role::Subtle))
        .child(
            Button::new("Start debugging", DEBUG_START)
                .size(Size::Sm)
                .build(),
        )
}

/// The body of the debug tab.
pub fn debug_tab(state: &WorkspaceState) -> Node {
    match &state.debug.run {
        Some(run) => node()
            .styles([flex(), flex_col(), flex_1(), min_h(0)])
            .child(controls(run))
            .child(
                node()
                    .styles([flex(), flex_1(), min_h(0)])
                    .child(stack(state, run))
                    .child(variables(state, run))
                    .child(console(state, run)),
            ),
        None => idle(state),
    }
}

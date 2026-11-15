//! The dock's Tasks tab: what the folder can be built, tested and run with,
//! each run in a new terminal tab when pressed.

use rustle::prelude::*;

use pm_components::{Label, LabelVariant, Leading, ListRow, Size};
use pm_shell::{TaskEntry, TaskList};

use crate::actions::Actions;

/// Emits the tasks of `list`, or a line saying why there are none.
pub fn tasks_panel(list: &TaskList, actions: &Actions) {
    let empty = if !list.available {
        Some("Open a session to list the tasks of its folder.")
    } else if !list.loaded {
        Some("Reading the tasks…")
    } else if list.tasks.is_empty() && list.problem.is_none() {
        Some("The folder has no tasks.")
    } else {
        None
    };
    if let Some(line) = empty {
        Label::new(line)
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        return;
    }
    let scroll = remember_scroll_state();
    column_with(
        Modifier::new().fill_max_size().vertical_scroll(&scroll),
        || {
            if let Some(problem) = &list.problem {
                Label::new(problem.clone())
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .show();
            }
            for (index, task) in list.tasks.iter().enumerate() {
                key(index, || task_row(index, task, actions));
            }
        },
    );
}

/// Emits the task at `index`, which runs when pressed.
fn task_row(index: usize, task: &TaskEntry, actions: &Actions) {
    let run = actions.run_task.clone();
    let glyph = match task.group.as_str() {
        "build" => phosphor::HAMMER,
        "test" => phosphor::FLASK,
        _ => phosphor::PLAY,
    };
    ListRow::new(task.name.clone())
        .leading(Leading::Icon(glyph))
        .note(task.command.clone())
        .end_column(task.source.clone(), 120.0)
        .size(Size::Xs)
        .on_click(move || run(index))
        .show();
}

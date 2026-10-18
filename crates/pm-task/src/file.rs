//! The tasks a project writes down itself.

use std::path::Path;

use serde::Deserialize;

use crate::task::Task;

/// The file a project writes its own tasks in, in the root of the project: `{"tasks": [{"name": "…", "command": "…"}]}`, each task with the fields of a [`Task`].
pub const FILE: &str = ".pandemonium.tasks.json";

/// What the file holds.
#[derive(Deserialize)]
struct Written {
    /// The tasks, in the order they are written.
    #[serde(default)]
    tasks: Vec<Task>,
}

/// The tasks written in the file of `root`, and why it cannot be read when it cannot; none and no problem when there is no file.
pub(crate) fn read(root: &Path) -> (Vec<Task>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(root.join(FILE)) else {
        return (Vec::new(), None);
    };
    match serde_json::from_str::<Written>(&text) {
        Ok(written) => {
            let tasks = written
                .tasks
                .into_iter()
                .filter(|task| !task.name.trim().is_empty() && !task.command.trim().is_empty())
                .map(|mut task| {
                    task.source = FILE.to_owned();
                    task
                })
                .collect();
            (tasks, None)
        }
        Err(error) => (Vec::new(), Some(format!("{FILE}: {error}"))),
    }
}

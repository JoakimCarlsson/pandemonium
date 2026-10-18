//! The tasks of a Go module.

use std::path::Path;

use crate::task::Task;

/// The tasks of the `go.mod` in `root`, or none when it has none.
pub(crate) fn detect(root: &Path) -> Vec<Task> {
    if !root.join("go.mod").is_file() {
        return Vec::new();
    }
    let task =
        |name: &str, command: &str, group: &str| Task::new(name, command, group, "go", "go.mod");
    let mut found = vec![
        task("go build", "go build ./...", "build"),
        task("go test", "go test ./...", "test"),
        task("go vet", "go vet ./...", "test"),
    ];
    if root.join("main.go").is_file() {
        found.push(task("go run", "go run .", "run"));
    }
    found
}

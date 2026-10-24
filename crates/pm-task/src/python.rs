//! The tasks of a Python project.

use std::path::Path;

use crate::task::Task;

/// The scripts a `pyproject.toml` installs, by name, from its `[project.scripts]` and its Poetry scripts.
fn scripts_of(pyproject: &toml::Table) -> Vec<String> {
    let project = pyproject
        .get("project")
        .and_then(|project| project.get("scripts"));
    let poetry = pyproject
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(|poetry| poetry.get("scripts"));
    [project, poetry]
        .into_iter()
        .flatten()
        .filter_map(|scripts| scripts.as_table())
        .flat_map(|scripts| scripts.keys().cloned())
        .collect()
}

/// Whether the project in `root`, whose `pyproject.toml` reads `pyproject`, is tested with pytest.
fn has_pytest(root: &Path, pyproject: &str) -> bool {
    pyproject.contains("pytest")
        || root.join("pytest.ini").is_file()
        || root.join("conftest.py").is_file()
        || root.join("tests").is_dir()
}

/// The tasks of the Python project in `root`, or none when it is not one.
pub(crate) fn detect(root: &Path) -> Vec<Task> {
    let pyproject_text = std::fs::read_to_string(root.join("pyproject.toml")).ok();
    let requirements = root.join("requirements.txt").is_file();
    if pyproject_text.is_none()
        && !requirements
        && !root.join("setup.py").is_file()
        && !root.join("main.py").is_file()
    {
        return Vec::new();
    }
    let task = |name: String, command: String, group: &str, source: &str| {
        Task::new(name, command, group, "python", source)
    };
    let mut found = Vec::new();
    if requirements {
        found.push(task(
            "pip install".to_owned(),
            "python -m pip install -r requirements.txt".to_owned(),
            "",
            "requirements.txt",
        ));
    } else if pyproject_text.is_some() {
        found.push(task(
            "pip install".to_owned(),
            "python -m pip install -e .".to_owned(),
            "",
            "pyproject.toml",
        ));
    }
    let text = pyproject_text.unwrap_or_default();
    if has_pytest(root, &text) {
        found.push(task(
            "pytest".to_owned(),
            "python -m pytest".to_owned(),
            "test",
            "pyproject.toml",
        ));
    }
    let table: toml::Table = text.parse().unwrap_or_default();
    found.extend(
        scripts_of(&table)
            .into_iter()
            .map(|script| task(format!("python: {script}"), script, "run", "pyproject.toml")),
    );
    if root.join("main.py").is_file() {
        found.push(task(
            "python main.py".to_owned(),
            "python main.py".to_owned(),
            "run",
            "main.py",
        ));
    }
    found
}

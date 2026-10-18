//! A task made ready to run: the folder, the command and the environment it starts with.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::task::Task;
use crate::toolchain::Toolchain;

/// How many of the last lines a failed task printed are kept to say why it failed.
const TAIL: usize = 12;

/// A task about to run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Launch {
    /// The folder it runs in.
    pub directory: PathBuf,
    /// What is typed into the shell.
    pub command: String,
    /// What the environment holds besides what the shell starts with: the toolchain's, then the task's own.
    pub env: BTreeMap<String, String>,
}

impl Launch {
    /// `task` made ready to run in the project at `root` with `toolchain`, or why it cannot: its folder is not inside the project.
    pub fn new(root: &Path, task: &Task, toolchain: Option<&Toolchain>) -> Result<Self, String> {
        let folder = Path::new(&task.folder);
        let escapes = folder.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
        if escapes {
            return Err(format!("{} runs outside the project", task.name));
        }
        let directory = root.join(folder);
        if !directory.is_dir() {
            return Err(format!("{} has no folder {}", task.name, task.folder));
        }
        let mut env = toolchain
            .map(|toolchain| toolchain.environment(std::env::var_os("PATH")))
            .unwrap_or_default();
        env.extend(task.env.clone());
        Ok(Self {
            directory,
            command: task.command.clone(),
            env,
        })
    }

    /// Runs the command to its end, and says why it failed when it did: how it ended, and the last lines it printed.
    pub fn finish(&self) -> Result<(), String> {
        let (program, flag) = if cfg!(windows) {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };
        let output = Command::new(program)
            .arg(flag)
            .arg(&self.command)
            .current_dir(&self.directory)
            .envs(&self.env)
            .output()
            .map_err(|error| format!("could not start {}: {error}", self.command))?;
        if output.status.success() {
            return Ok(());
        }
        let printed = [output.stdout, output.stderr].concat();
        let text = String::from_utf8_lossy(&printed);
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        let tail = lines[lines.len().saturating_sub(TAIL)..].join("\n");
        Err(format!("{} ({})\n{tail}", self.command, output.status))
    }
}

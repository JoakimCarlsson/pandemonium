//! The conversation of a session as the harness saved it, read and named without an agent running.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use omc_client::{Conversation, Imported};

use crate::agent::harness_program;

/// Runs the harness with `arguments` and returns what it printed.
///
/// # Errors
///
/// Fails with the reason when the harness is not installed, cannot be started, or fails.
fn run(arguments: &[&std::ffi::OsStr], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let program = harness_program().ok_or("the harness is not installed")?;
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start the harness: {error}"))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(input)
            .map_err(|error| format!("could not hand the harness its input: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("the harness did not finish: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(output.stdout)
}

/// The conversation saved under `id` in `folder`: its name, the choices made about it and its turns, oldest first; an empty one when nothing was saved.
///
/// # Errors
///
/// Fails with the reason when the harness is not installed, cannot be started, or does not answer with a conversation.
pub fn saved_conversation(id: &str, folder: &Path) -> Result<Conversation, String> {
    let printed = run(&["history".as_ref(), id.as_ref(), folder.as_os_str()], None)?;
    serde_json::from_slice(&printed)
        .map_err(|error| format!("the harness told the history in a form not understood: {error}"))
}

/// Names the conversation saved under `id` in `folder` `title`, for a conversation no agent has open.
///
/// # Errors
///
/// Fails with the reason when the harness is not installed, cannot be started, or cannot write the name, as when an agent has the conversation open.
pub fn rename_saved(id: &str, folder: &Path, title: &str) -> Result<(), String> {
    run(
        &[
            "rename".as_ref(),
            id.as_ref(),
            folder.as_os_str(),
            title.as_ref(),
        ],
        None,
    )
    .map(|_| ())
}

/// Hands the harness the conversation `imported`, which a client kept itself, to save under `id` in `folder` as its own.
///
/// # Errors
///
/// Fails with the reason when the harness is not installed, cannot be started, or refuses, as when it already holds a history for `id`.
pub fn import_saved(id: &str, folder: &Path, imported: &Imported) -> Result<(), String> {
    let text = serde_json::to_vec(imported).map_err(|error| error.to_string())?;
    run(
        &["import".as_ref(), id.as_ref(), folder.as_os_str()],
        Some(&text),
    )
    .map(|_| ())
}

#[cfg(test)]
/// Tests that ask the real harness.
mod tests {
    use super::{rename_saved, saved_conversation};

    /// Nothing saved reads as an empty conversation.
    #[test]
    #[ignore = "needs the harness installed"]
    fn live_harness_reads_an_empty_history() {
        let folder =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/history-live");
        let saved = saved_conversation("nothing-here", &folder).unwrap();
        assert!(saved.turns.is_empty() && saved.title.is_none());
    }

    /// A conversation named while no agent has it open comes back with its name.
    #[test]
    #[ignore = "needs the harness installed"]
    fn live_harness_names_a_saved_conversation() {
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/history-live-rename");
        let _ = std::fs::remove_dir_all(&folder);
        rename_saved("named", &folder, "Fix the thing").unwrap();
        let saved = saved_conversation("named", &folder).unwrap();
        assert_eq!(saved.title.as_deref(), Some("Fix the thing"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}

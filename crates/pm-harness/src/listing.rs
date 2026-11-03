//! The tools the harness can give an agent, as the harness reports them.

use std::process::Command;

use pm_agent_library::ToolEntry;

use crate::agent::harness_program;

/// Asks the harness which tools it can give an agent.
///
/// # Errors
///
/// Fails with the reason when the harness is not installed, cannot be started, or does not answer with a list of tools.
pub fn harness_tools() -> Result<Vec<ToolEntry>, String> {
    let program = harness_program().ok_or("the harness is not installed")?;
    let output = Command::new(program)
        .arg("tools")
        .output()
        .map_err(|error| format!("could not start the harness: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let listed: Vec<omc_client::ToolInfo> =
        serde_json::from_slice(&output.stdout).map_err(|error| {
            format!("the harness listed its tools in a form not understood: {error}")
        })?;
    Ok(listed
        .into_iter()
        .map(|info| ToolEntry {
            name: info.name,
            description: info.description,
        })
        .collect())
}

#[cfg(test)]
/// Tests that ask the real harness.
mod tests {
    use super::harness_tools;

    /// The harness lists its tools in a form the app reads.
    #[test]
    #[ignore = "needs the harness installed"]
    fn live_harness_lists_its_tools() {
        let listed = harness_tools().unwrap();
        assert!(listed.iter().any(|tool| tool.name == "bash"));
        assert!(listed.iter().all(|tool| !tool.description.is_empty()));
    }
}

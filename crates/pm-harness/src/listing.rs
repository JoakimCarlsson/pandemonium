//! The tools the harness can give an agent.

use pm_agent_library::ToolEntry;

/// The tools the harness can give an agent.
///
/// # Errors
///
/// Fails with the reason when the tools cannot be made.
pub fn harness_tools() -> Result<Vec<ToolEntry>, String> {
    Ok(omc_session::tools()?
        .into_iter()
        .map(|info| ToolEntry {
            name: info.name,
            description: info.description,
        })
        .collect())
}

//! The model servers the user declares for the harness: OpenAI-compatible servers such as a company gateway or a local vLLM, each offered as an agent of its own once it is saved.

use omc_wire::Server;

use crate::agent::refresh_agents;

/// The model servers declared on this machine, in the order they are offered, without their keys.
///
/// # Errors
///
/// Fails with the reason when the harness's configuration cannot be read or is not valid.
pub fn model_servers() -> Result<Vec<Server>, String> {
    omc_session::servers()
}

/// Adds `server` to the harness's configuration, or puts it in place of the one with the same id, offers it as an agent from now on, and returns the servers as they then stand.
///
/// A server sent without a key keeps the one saved for its id, and one sent with an empty key has none.
///
/// # Errors
///
/// Fails with the reason, changing nothing, when the server is not valid or the configuration cannot be read or written.
pub fn save_model_server(server: Server) -> Result<Vec<Server>, String> {
    let saved = omc_session::save_server(server)?;
    refresh_agents();
    Ok(saved)
}

/// Takes the server `id` out of the harness's configuration, stops offering it as an agent, and returns the servers left.
///
/// # Errors
///
/// Fails with the reason when no server has the id or the configuration cannot be read or written.
pub fn remove_model_server(id: &str) -> Result<Vec<Server>, String> {
    let left = omc_session::remove_server(id)?;
    refresh_agents();
    Ok(left)
}

/// The models `server` offers, saved or not, asked now and waited for: those it was given, or those it lists at `GET {base_url}/models`. A server sent without a key is asked with the one saved for its id.
///
/// # Errors
///
/// Fails with the reason when the server is not valid, cannot be reached or answers with an error.
pub fn find_models(server: Server) -> Result<Vec<String>, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(omc_session::find_models(server))
}

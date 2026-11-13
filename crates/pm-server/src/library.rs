//! What agents are given in a project, read where the session runs, and what the reader turned off of it.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use pm_agent_library::{Item, Kind, Source, ToolEntry, ToolSet};
use pm_harness::Agent;
use pm_protocol::McpServer;

/// What the keys of everything turned off are kept under.
pub(crate) const WITHHELD: &str = "withheld";

/// The tools each tool server listed, by how it is reached, so that it is started only the first time they are asked for.
static LISTED: Mutex<Option<HashMap<String, Vec<ToolEntry>>>> = Mutex::new(None);

/// The keys of every customization, tool set and tool the reader turned off.
pub(crate) fn withheld() -> Vec<String> {
    pm_store::load_text(WITHHELD)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Every customization the harness gives an agent in the project at `directory`, turned off or not; only the reader's own and the built-in ones when `directory` is empty, as no project is open.
pub(crate) fn items(directory: &Path) -> Vec<Item> {
    if directory.as_os_str().is_empty() {
        return pm_agent_library::discover_own();
    }
    pm_agent_library::discover(directory)
}

/// The tool servers agents are started with that are switched on: those saved and those the extensions bring.
fn servers() -> Vec<McpServer> {
    serde_json::from_str::<Vec<McpServer>>(&crate::extensions::mcp_text())
        .unwrap_or_default()
        .into_iter()
        .filter(|server| server.enabled)
        .collect()
}

/// The set of `server`'s tools, with none listed yet.
fn set_of(server: &McpServer) -> ToolSet {
    ToolSet {
        id: server.name.clone(),
        name: server.name.clone(),
        source: match server.extension {
            Some(_) => Source::Plugin,
            None => Source::Server,
        },
        plugin: server.extension.clone(),
        ..ToolSet::default()
    }
}

/// `server` as the client of tool servers reaches it.
fn reached(server: &McpServer) -> Option<pm_mcp::Server> {
    if server.target.starts_with("http://") || server.target.starts_with("https://") {
        return Some(pm_mcp::Server::Address {
            url: server.target.clone(),
            headers: server.env.clone(),
        });
    }
    let mut words = pm_harness::command_words(&server.target).into_iter();
    Some(pm_mcp::Server::Command {
        program: words.next()?,
        args: words.collect(),
        env: server.env.clone(),
    })
}

/// The tools `server` offers, as it listed them the first time it was asked, or why they could not be listed.
fn listed(server: &McpServer) -> Result<Vec<ToolEntry>, String> {
    let key = format!("{}\0{:?}", server.target, server.env);
    if let Some(tools) = LISTED
        .lock()
        .ok()
        .and_then(|held| held.as_ref()?.get(&key).cloned())
    {
        return Ok(tools);
    }
    let reached = reached(server).ok_or("the server has no command")?;
    let tools: Vec<ToolEntry> = pm_mcp::tools(&reached)?
        .into_iter()
        .map(|tool| ToolEntry {
            name: tool.name,
            description: tool.description,
        })
        .collect();
    if let Ok(mut held) = LISTED.lock() {
        held.get_or_insert_with(HashMap::new)
            .insert(key, tools.clone());
    }
    Ok(tools)
}

/// The tools `agent` is given: its own, then each tool server's, every server asked at once.
pub(crate) fn tool_sets(agent: Agent) -> Vec<ToolSet> {
    let servers = servers();
    let listed: Vec<ToolSet> = std::thread::scope(|scope| {
        let asked: Vec<_> = servers
            .iter()
            .map(|server| scope.spawn(move || (set_of(server), listed(server))))
            .collect();
        asked
            .into_iter()
            .filter_map(|asked| asked.join().ok())
            .map(|(set, answer)| match answer {
                Ok(tools) => ToolSet { tools, ..set },
                Err(reason) => ToolSet {
                    error: Some(reason),
                    ..set
                },
            })
            .collect()
    });
    ToolSet::built_in(agent.product(), pm_harness::harness_tools())
        .into_iter()
        .chain(listed)
        .collect()
}

/// Whether the reader turned off the built-in set or any tool of it, which the harness can only be told by name once it has listed its tools.
fn keeps_built_in_off(set: &ToolSet, withheld: &[String]) -> bool {
    let prefix = format!("tool/{}/", set.id);
    set.off(withheld) || withheld.iter().any(|key| key.starts_with(&prefix))
}

/// The instruction files (by path), skills and agent definitions (by name) of the project at `directory` that the reader turned off in `withheld`.
fn disabled(agent: Agent, directory: &Path, withheld: &[String]) -> Vec<String> {
    items(directory)
        .into_iter()
        .filter(|item| item.kind.switchable(agent.product()) && !item.enabled(withheld))
        .filter_map(|item| match item.kind {
            Kind::Instruction => Some(item.path),
            Kind::Skill | Kind::Agent => Some(item.name),
            Kind::Hook => None,
        })
        .collect()
}

/// What `agent` of the session numbered `session` working in `directory` is given: the tool servers the reader keeps on, the browser of the session and the tools that start sessions, and the names of the tools the reader turned off.
pub(crate) fn given(agent: Agent, directory: &Path, session: u64) -> pm_harness::Given {
    let withheld = withheld();
    let kept: Vec<McpServer> = servers()
        .into_iter()
        .filter(|server| !set_of(server).off(&withheld))
        .collect();
    let listed = pm_harness::harness_tools();
    let built_in = ToolSet::built_in(agent.product(), listed.clone());
    let blocked = match (&listed, built_in.as_ref()) {
        (Err(reason), Some(set)) if keeps_built_in_off(set, &withheld) => Some(format!(
            "the agent was not started because the harness could not list its tools, so what you turned off could not be kept off: {reason}"
        )),
        _ => None,
    };
    let mut denied: Vec<String> = built_in
        .map(|set| set.withheld_names(&withheld))
        .unwrap_or_default();
    for server in &kept {
        let set = set_of(server);
        let prefix = format!("tool/{}/", set.id);
        denied.extend(
            withheld
                .iter()
                .filter_map(|key| key.strip_prefix(&prefix))
                .map(|tool| set.agent_name(tool)),
        );
    }
    let mut tools = pm_harness::mcp_servers(&serde_json::to_string(&kept).unwrap_or_default());
    if let Some((url, token)) = crate::browser_tools::offered(session) {
        tools.push(pm_harness::ToolServer {
            name: "pandemonium-browser".to_owned(),
            reach: pm_harness::Reach::Http {
                url,
                headers: vec![("Authorization".to_owned(), format!("Bearer {token}"))],
            },
        });
    }
    if let Some((url, token)) = crate::session_tools::offered(session) {
        tools.push(pm_harness::ToolServer {
            name: "pandemonium-sessions".to_owned(),
            reach: pm_harness::Reach::Http {
                url,
                headers: vec![("Authorization".to_owned(), format!("Bearer {token}"))],
            },
        });
    }
    pm_harness::Given {
        tools,
        denied,
        disabled: disabled(agent, directory, &withheld),
        blocked,
        ..pm_harness::Given::default()
    }
}

#[cfg(test)]
/// Tests for what an agent is kept from when the harness cannot list its tools.
mod tests {
    use super::*;
    use pm_agent_library::Product;

    /// Turning off the built-in set or one of its tools means the listing is needed; turning off another set's tool does not.
    #[test]
    fn built_in_switches_need_the_listing() {
        let set = ToolSet::built_in(Some(Product::Claude), Err("down".to_owned())).unwrap();
        assert!(!keeps_built_in_off(&set, &[]));
        assert!(!keeps_built_in_off(
            &set,
            &["tool/github/search".to_owned()]
        ));
        assert!(keeps_built_in_off(&set, &["tool/built-in/bash".to_owned()]));
        assert!(keeps_built_in_off(&set, &["tools/built-in".to_owned()]));
    }
}

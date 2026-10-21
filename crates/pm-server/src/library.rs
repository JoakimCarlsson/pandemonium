//! What agents are given in a project, read where the session runs, and what the reader turned off of it.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use pm_acp::Agent;
use pm_agent_library::{Item, Launch, Source, ToolEntry, ToolSet};
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

/// Every customization `agent` is given in the project at `directory`, turned off or not.
pub(crate) fn items(agent: Agent, directory: &Path) -> Vec<Item> {
    pm_agent_library::discover(agent.product(), directory, agent.config_folder().as_deref())
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
    let mut words = pm_acp::command_words(&server.target).into_iter();
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
    ToolSet::built_in(agent.product())
        .into_iter()
        .chain(listed)
        .collect()
}

/// What `agent` is started with in `directory`: the tool servers it is given, and what leaves out the customizations and tools the reader turned off.
pub(crate) fn given(agent: Agent, directory: &Path) -> (Vec<serde_json::Value>, Launch) {
    let withheld = withheld();
    let product = agent.product();
    let kept: Vec<McpServer> = servers()
        .into_iter()
        .filter(|server| !set_of(server).off(&withheld))
        .collect();
    let mut tools: Vec<String> = ToolSet::built_in(product)
        .map(|set| set.withheld_names(&withheld))
        .unwrap_or_default();
    for server in &kept {
        let set = set_of(server);
        let prefix = format!("tool/{}/", set.id);
        tools.extend(
            withheld
                .iter()
                .filter_map(|key| key.strip_prefix(&prefix))
                .map(|tool| set.agent_name(tool)),
        );
    }
    let off: Vec<Item> = items(agent, directory)
        .into_iter()
        .filter(|item| !item.enabled(&withheld))
        .collect();
    (
        pm_acp::mcp_servers(&serde_json::to_string(&kept).unwrap_or_default()),
        Launch::without(product, &off, &tools),
    )
}

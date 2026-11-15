//! The settings the snapshots render: what a server with a few tool
//! servers, model servers, slots, languages and customizations would tell,
//! without a server.

use pm_protocol::library::{Item, Kind, Scope, ToolEntry, ToolSet};
use pm_protocol::{ExtensionInfo, McpServer};

use crate::settings_model::{Release, SettingsModel};
use crate::settings_section::Section;

/// A language extension named `name`, installed or not.
fn language(id: &str, name: &str, installed: bool, description: &str) -> ExtensionInfo {
    ExtensionInfo {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: "language".to_owned(),
        version: "1.0.0".to_owned(),
        description: description.to_owned(),
        installed,
        ..ExtensionInfo::default()
    }
}

/// A tool server named `name` started with `target`.
fn tool_server(name: &str, target: &str, description: &str, enabled: bool) -> McpServer {
    McpServer {
        name: name.to_owned(),
        target: target.to_owned(),
        env: Vec::new(),
        description: description.to_owned(),
        website: String::new(),
        enabled,
        extension: None,
    }
}

/// A customization of `kind` named `name`.
fn item(kind: Kind, scope: Scope, name: &str, description: &str) -> Item {
    Item {
        kind,
        scope,
        name: name.to_owned(),
        description: description.to_owned(),
        path: format!("~/.claude/{name}"),
        ..Item::default()
    }
}

/// A model slot named `name` set to `model`, falling back to `backups`.
fn slot(name: &str, model: Option<&str>, backups: &[&str]) -> pm_harness::SlotInfo {
    pm_harness::SlotInfo {
        name: name.to_owned(),
        model: model.map(str::to_owned),
        backups: backups.iter().map(|backup| (*backup).to_owned()).collect(),
        resolved: model.map(str::to_owned),
        source: if model.is_some() {
            pm_harness::SlotSource::User
        } else {
            pm_harness::SlotSource::Main
        },
        inherited: model.is_none(),
        suggested: None,
        providers: Vec::new(),
    }
}

/// The settings window's model open on `section`, filled as a server with a little of everything would fill it.
pub fn sample_settings(section: Section) -> SettingsModel {
    let mut model = SettingsModel::new(Default::default(), Default::default());
    model.machine_loaded = true;
    if section == Section::Results {
        model.page.find = "font".to_owned();
    } else {
        model.page.section = section;
    }
    model.release = Some(Release::Current);
    model.fonts = ["Inter", "JetBrains Mono", "Noto Sans", "Fira Code"]
        .map(str::to_owned)
        .to_vec();
    model.mono_fonts = ["JetBrains Mono", "Fira Code"].map(str::to_owned).to_vec();
    model.extensions = vec![
        language("rust", "Rust", true, "Rust, with rust-analyzer."),
        language("toml", "TOML", true, "TOML, with taplo."),
        language("python", "Python", false, "Python, with basedpyright."),
        language("go", "Go", false, "Go, with gopls."),
    ];
    model.extensions_loaded = true;
    model.mcp_servers = vec![
        tool_server(
            "github",
            "npx -y @modelcontextprotocol/server-github",
            "Issues, pull requests and code search on GitHub.",
            true,
        ),
        tool_server(
            "postgres",
            "npx -y @modelcontextprotocol/server-postgres",
            "Read-only queries against a database.",
            false,
        ),
    ];
    model.mcp_loaded = true;
    model.model_servers_loaded = true;
    model.slots = vec![
        slot("fast", Some("claude-haiku-4-5"), &["gpt-5-mini"]),
        slot("smart", Some("claude-opus-4-1"), &[]),
        slot("vision", None, &[]),
    ];
    model.slots_loaded = true;
    model.library_place = Some(("claude".to_owned(), "~/repos/pandemonium".to_owned()));
    model.library = vec![
        item(
            Kind::Skill,
            Scope::User,
            "release-notes",
            "Writes release notes from the commits since the last tag.",
        ),
        item(
            Kind::Skill,
            Scope::Workspace,
            "snapshot",
            "Renders the window and inspects the image.",
        ),
        item(
            Kind::Instruction,
            Scope::Workspace,
            "AGENTS.md",
            "Architecture rules and Rust conventions.",
        ),
        item(
            Kind::Agent,
            Scope::User,
            "reviewer",
            "Reviews a change for quality and security.",
        ),
        item(
            Kind::Hook,
            Scope::User,
            "format",
            "Runs cargo fmt after edits.",
        ),
    ];
    model.withheld = vec!["skill/workspace/snapshot".to_owned()];
    model.tool_sets = vec![ToolSet {
        id: "built-in".to_owned(),
        name: "Built-in".to_owned(),
        tools: ["read", "edit", "bash", "grep"]
            .map(|name| ToolEntry {
                name: name.to_owned(),
                description: format!("The agent's own {name} tool."),
            })
            .to_vec(),
        ..ToolSet::default()
    }];
    model.library_loaded = true;
    model.window.servers.known.push(pm_config::KnownServer {
        name: "Lab".to_owned(),
        reach: pm_config::Reach::Ssh,
        address: "me@lab.local".to_owned(),
        ..pm_config::KnownServer::default()
    });
    model
}

/// Opens on `model` the form of the section it shows, filled in as a reader
/// would have it: an agent's key and accounts, the registry's results and the
/// tool server form, the model server form with the models found, a slot
/// being edited, or the draft of a new skill.
pub fn sample_forms(model: &mut SettingsModel) {
    use crate::settings_forms::{Draft, McpForm, ModelForm};
    let forms = &mut model.forms;
    match model.page.section {
        Section::AgentServers => {
            forms.agent = model.agents.first().map(|agent| agent.id.clone());
            let agent = forms.agent.clone().unwrap_or_default();
            forms.key = "sk-ant-api03-••••••••".to_owned();
            forms.logins = vec![
                pm_protocol::Login {
                    agent: agent.clone(),
                    id: format!("{agent}-work"),
                    name: "Work".to_owned(),
                },
                pm_protocol::Login {
                    agent: agent.clone(),
                    id: format!("{agent}-personal"),
                    name: "Personal".to_owned(),
                },
            ];
            forms.in_use = vec![(agent.clone(), format!("{agent}-work"))];
        }
        Section::McpServers => {
            forms.query = "browser".to_owned();
            forms.listings = vec![
                pm_protocol::ToolListing {
                    id: "io.github.microsoft/playwright-mcp".to_owned(),
                    title: "Playwright".to_owned(),
                    description: "Drives a browser through Playwright's accessibility tree."
                        .to_owned(),
                    name: "playwright".to_owned(),
                    target: "npx -y @playwright/mcp".to_owned(),
                    env: Vec::new(),
                    inputs: Vec::new(),
                    asks: false,
                    website: String::new(),
                },
                pm_protocol::ToolListing {
                    id: "io.github.browserbase/mcp".to_owned(),
                    title: "Browserbase".to_owned(),
                    description: "Cloud browsers for agents.".to_owned(),
                    name: "browserbase".to_owned(),
                    target: "npx -y @browserbasehq/mcp".to_owned(),
                    env: Vec::new(),
                    inputs: vec!["BROWSERBASE_API_KEY".to_owned()],
                    asks: false,
                    website: String::new(),
                },
            ];
            forms.mcp = Some(McpForm {
                name: "files".to_owned(),
                target: "npx -y @modelcontextprotocol/server-filesystem ~/repos".to_owned(),
                env: String::new(),
            });
        }
        Section::ModelServers => {
            forms.model = Some(ModelForm {
                id: "local-vllm".to_owned(),
                label: "Local vLLM".to_owned(),
                base_url: "http://localhost:8000/v1".to_owned(),
                key: String::new(),
                default_model: "qwen3-coder-30b".to_owned(),
                models: vec!["qwen3-coder-30b".to_owned(), "gpt-oss-20b".to_owned()],
                finding: false,
            });
        }
        Section::ModelSlots => {
            forms.slot = model.slots.first().map(|slot| {
                (
                    slot.name.clone(),
                    "anthropic/claude-opus-5-5:high".to_owned(),
                    "openai/gpt-5.5".to_owned(),
                )
            });
        }
        Section::Skills => {
            forms.draft = Some(Draft {
                kind: Kind::Skill,
                name: "release-notes".to_owned(),
                description: "Writes the release notes from the merged pull requests.".to_owned(),
                personal: false,
            });
        }
        _ => {}
    }
}

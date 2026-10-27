//! The extensions installed on the server's machine: what each is allowed, what it brings that the server acts on, and running its commands.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pm_plugin::Package;
use pm_policy::{Capability, Grants};
use pm_protocol::{
    ExtensionAssets, ExtensionCommand, ExtensionIcon, ExtensionIconTheme, ExtensionInfo,
    ExtensionTheme, McpServer, Response,
};

use crate::core::Core;
use crate::handler::{failed, place};

/// The capabilities the reader granted the extension `id`.
pub(crate) fn grants_of(id: &str) -> Grants {
    let granted = crate::machine::settings().granted(id);
    Grants::new(id, granted.iter().map(String::as_str))
}

/// Whether the tool server `target` is reached over the network rather than started as a program.
fn is_address(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

/// The reason an extension that was not granted `capability` is refused, in words for the reader.
fn refusal(package: &Package, capability: Capability) -> String {
    grants_of(&package.id)
        .check(capability)
        .err()
        .map(|denied| denied.to_string())
        .unwrap_or_default()
}

/// How an extension package is listed: what it carries, what it asks for and what it was granted.
pub(crate) fn describe(package: &Package, installed: bool) -> ExtensionInfo {
    let manifest = &package.manifest;
    let mut provides = Vec::new();
    provides.extend(package.languages().iter().map(|(language, folder)| {
        let name =
            pm_lsp::Manifest::read(folder).map_or_else(|_| language.clone(), |read| read.name);
        format!("language: {name}")
    }));
    provides.extend(
        package
            .themes()
            .iter()
            .map(|theme| format!("theme: {}", theme.name)),
    );
    provides.extend(
        package
            .icon_themes()
            .iter()
            .map(|theme| format!("icon theme: {}", theme.name)),
    );
    provides.extend(
        manifest
            .debug_adapters
            .iter()
            .map(|adapter| format!("debug adapter: {}", adapter.name)),
    );
    provides.extend(
        manifest
            .mcp_servers
            .iter()
            .map(|server| format!("tool server: {}", server.name)),
    );
    provides.extend(
        manifest
            .commands
            .iter()
            .map(|command| format!("command: {}", command.title)),
    );
    ExtensionInfo {
        id: package.id.clone(),
        name: manifest.name.clone(),
        kind: "extension".to_owned(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        installed,
        local: true,
        provides,
        requests: manifest.capabilities.clone(),
        granted: if installed {
            grants_of(&package.id).ids()
        } else {
            Vec::new()
        },
        ..ExtensionInfo::default()
    }
}

/// The debug adapters the installed extensions bring, each refused unless its extension may run programs.
fn adapters() -> Vec<pm_dap::Adapter> {
    pm_plugin::installed()
        .into_iter()
        .flat_map(|package| {
            let denied = grants_of(&package.id)
                .check(Capability::Process)
                .err()
                .map(|denied| denied.to_string());
            package
                .manifest
                .debug_adapters
                .into_iter()
                .map(move |entry| pm_dap::Adapter {
                    id: entry.id,
                    name: entry.name,
                    programs: entry.programs,
                    arguments: entry.arguments,
                    connect: if entry.connect == "tcp" {
                        pm_dap::Connect::Tcp
                    } else {
                        pm_dap::Connect::Stdio
                    },
                    types: entry.types.iter().map(|name| name.to_lowercase()).collect(),
                    denied: denied.clone(),
                })
        })
        .collect()
}

/// The tool servers the installed extensions bring, listed with the extension that brings each; one that is started as a program is switched off unless its extension may run programs, and one reached by address unless it may download.
pub(crate) fn mcp_servers() -> Vec<McpServer> {
    pm_plugin::installed()
        .into_iter()
        .flat_map(|package| {
            let grants = grants_of(&package.id);
            let extension = package.id.clone();
            package
                .manifest
                .mcp_servers
                .into_iter()
                .map(move |entry| McpServer {
                    enabled: if is_address(&entry.target) {
                        grants.allows(Capability::Download)
                    } else {
                        grants.allows(Capability::Process)
                    },
                    name: entry.name,
                    target: entry.target,
                    env: entry.env.into_iter().collect(),
                    description: entry.description,
                    website: String::new(),
                    extension: Some(extension.clone()),
                })
        })
        .collect()
}

/// The tool servers every agent is started with, as the text the server keeps them in: those saved and those the extensions bring.
pub(crate) fn mcp_text() -> String {
    let mut servers: Vec<McpServer> = pm_store::load_text("mcp-servers")
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    servers.retain(|server| server.extension.is_none());
    servers.extend(mcp_servers());
    serde_json::to_string(&servers).unwrap_or_default()
}

/// What the installed extensions bring that a window uses itself.
pub(crate) fn assets() -> ExtensionAssets {
    let mut assets = ExtensionAssets::default();
    for package in pm_plugin::installed() {
        assets
            .themes
            .extend(package.themes().into_iter().map(|theme| ExtensionTheme {
                extension: theme.extension,
                name: theme.name,
                dark: theme.dark,
                light: theme.light,
            }));
        assets
            .icon_themes
            .extend(package.icon_themes().into_iter().map(|theme| {
                ExtensionIconTheme {
                    extension: theme.extension,
                    name: theme.name,
                    icons: theme
                        .icons
                        .into_iter()
                        .map(|icon| ExtensionIcon {
                            suffix: icon.suffix,
                            svg: icon.svg,
                            color: icon.color,
                        })
                        .collect(),
                }
            }));
        assets.commands.extend(
            package
                .manifest
                .commands
                .iter()
                .map(|command| ExtensionCommand {
                    extension: package.id.clone(),
                    id: command.id.clone(),
                    title: command.title.clone(),
                    description: command.description.clone(),
                }),
        );
    }
    assets
}

/// The languages brought by installed extensions, each with the extension's name, for listing them under it and refusing their servers.
pub(crate) fn language_owners() -> HashMap<String, Package> {
    pm_plugin::installed()
        .into_iter()
        .flat_map(|package| {
            package
                .languages()
                .into_iter()
                .map(|(language, _)| (language, package.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Whether what the language `language` declares may run programs on this machine: it is not carried by an extension, or the extension was granted `process`. An extension's data must not run what its module could not.
pub(crate) fn may_run_programs(language: &str) -> bool {
    owner_of(language).is_none_or(|package| grants_of(&package.id).allows(Capability::Process))
}

/// The installed extension that carries the language `language`, when one does.
pub(crate) fn owner_of(language: &str) -> Option<Package> {
    language_owners().remove(language)
}

/// What the module of `package` answers to `call`, which is about one of its languages, in `project` when it is about one; nothing when the module does not answer it.
pub(crate) fn language_hook(
    package: &Package,
    call: &pm_plugin_api::LanguageCall,
    project: Option<&std::path::Path>,
) -> Option<Result<pm_plugin_api::LanguageReply, String>> {
    match pm_plugin::run_language(package, call, project, &grants_of(&package.id)) {
        Ok(pm_plugin_api::LanguageReply::Unsupported) => None,
        other => Some(other),
    }
}

/// The folder of `project` as a module may be told it: nothing unless the extension `package` may read the project.
pub(crate) fn project_for(package: &Package, project: &std::path::Path) -> Option<String> {
    grants_of(&package.id)
        .allows(Capability::ReadProject)
        .then(|| project.to_string_lossy().into_owned())
}

/// What each language that an extension brought is refused: its language server is not started unless the extension may run programs.
pub(crate) fn refused_servers() -> HashMap<String, pm_lsp::ServerChoice> {
    language_owners()
        .into_iter()
        .filter(|(_, package)| !grants_of(&package.id).allows(Capability::Process))
        .map(|(language, package)| {
            let reason = refusal(&package, Capability::Process);
            (language, pm_lsp::ServerChoice::Refused(reason))
        })
        .collect()
}

impl Core {
    /// Takes in what changed about extensions: the adapters and language servers they bring are refused or allowed as the grants say, and every client is told how the extensions stand.
    pub(crate) fn extensions_changed(&self) {
        pm_dap::set_extension_adapters(adapters());
        self.documents.settings_changed(&crate::machine::settings());
        self.tell_extensions();
    }

    /// Installs the extension package in the folder `path` with the capabilities `grants` allowed, on a thread of its own, telling every client how it goes.
    pub(crate) fn install_package(self: &Arc<Self>, path: &str, grants: Vec<String>) -> Response {
        let source = place(path);
        let package = match pm_plugin::inspect(&source) {
            Ok(package) => package,
            Err(reason) => return failed(reason),
        };
        let id = package.id.clone();
        let allowed: Vec<String> = package
            .manifest
            .capabilities
            .iter()
            .filter(|asked| grants.contains(asked))
            .cloned()
            .collect();
        let mut settings = crate::machine::settings();
        settings.set_granted(&id, allowed);
        if let Response::Failed { message } = crate::machine::save(&settings) {
            return failed(message);
        }
        let granted = grants_of(&id);
        let core = self.clone();
        self.note_busy_package(&id, Some("Installing"));
        std::thread::spawn(move || {
            let progress = |_: &str, step: &str| core.note_busy_package(&id, Some(step));
            let outcome =
                pm_plugin::install(&source, &granted, &|| core.extensions_changed(), &progress);
            core.note_problem_package(
                &id,
                match outcome {
                    Ok(done) if done.problems.is_empty() => None,
                    Ok(done) => Some(done.problems.join("; ")),
                    Err(reason) => Some(reason),
                },
            );
            core.note_busy_package(&id, None);
            core.extensions_changed();
        });
        Response::Done
    }

    /// Takes the extension package `id` off this machine, with what it was granted.
    pub(crate) fn remove_package(&self, id: &str) -> Result<(), String> {
        pm_plugin::remove(id)?;
        let mut settings = crate::machine::settings();
        settings.set_granted(id, Vec::new());
        if let Response::Failed { message } = crate::machine::save(&settings) {
            return Err(message);
        }
        self.note_problem_package(id, None);
        self.extensions_changed();
        Ok(())
    }

    /// Allows the installed extension `id` the capability `capability`, or takes it back; allowing downloads installs what the extension could not before for want of them.
    pub(crate) fn grant(
        self: &Arc<Self>,
        id: &str,
        capability: &str,
        granted: bool,
    ) -> Result<(), String> {
        let package = pm_plugin::named(id).ok_or_else(|| format!("{id} is not installed"))?;
        if Capability::parse(capability).is_none() {
            return Err(format!("{capability} is no capability"));
        }
        if !package
            .manifest
            .capabilities
            .iter()
            .any(|asked| asked == capability)
        {
            return Err(format!(
                "{} does not ask for {capability}",
                package.manifest.name
            ));
        }
        let mut settings = crate::machine::settings();
        let mut held = settings.granted(id);
        held.retain(|name| name != capability);
        if granted {
            held.push(capability.to_owned());
        }
        settings.set_granted(id, held.clone());
        if let Response::Failed { message } = crate::machine::save(&settings) {
            return Err(message);
        }
        self.extensions_changed();
        if granted && capability == Capability::Download.id() {
            let kept = package.folder.to_string_lossy().into_owned();
            if let Response::Failed { message } = self.install_package(&kept, held) {
                return Err(message);
            }
        }
        Ok(())
    }

    /// Runs the command `command` of the extension `extension`, in the folder `directory` when it runs in one.
    pub(crate) fn run_extension_command(
        &self,
        extension: &str,
        command: &str,
        argument: &str,
        directory: Option<&str>,
    ) -> Response {
        let Some(package) = pm_plugin::named(extension) else {
            return failed(format!("{extension} is not installed"));
        };
        let project: Option<PathBuf> = directory.map(place);
        match pm_plugin::run_command(
            &package,
            command,
            argument,
            project.as_deref(),
            &grants_of(extension),
        ) {
            Ok(reply) => Response::ExtensionCommand {
                prompt: reply.prompt,
                message: reply.message,
            },
            Err(reason) => failed(reason),
        }
    }
}

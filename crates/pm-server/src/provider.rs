//! Asking the code of the extensions that brought languages and debug adapters: which program starts a language server or a debug adapter, what the server is told, and what its completions and symbols are called in a list.

use std::path::Path;
use std::sync::{Arc, Weak};

use pm_lsp::{Completion, Provided, ProvidedCommand, Provider, ServerSpec, SymbolLabel};
use pm_plugin::Package;
use pm_plugin_api::{CompletionItem, HookCall, HookReply, SymbolItem};
use pm_policy::Capability;

use crate::core::Core;
use crate::extensions::{grants_of, language_owners};

/// What asks the extensions of one server, which tells every client what an extension says it is doing.
pub(crate) struct Extensions {
    /// The server, which hears what the extensions say they are doing.
    core: Weak<Core>,
}

impl Extensions {
    /// The asker for the extensions of `core`.
    pub(crate) fn new(core: &Arc<Core>) -> Arc<Self> {
        Arc::new(Self {
            core: Arc::downgrade(core),
        })
    }

    /// What is done with a line the extension `id` says about what it is doing.
    fn status_of(&self, id: &str) -> pm_plugin::Status {
        let (core, id) = (self.core.clone(), id.to_owned());
        Arc::new(move |text: &str| {
            if let Some(core) = core.upgrade() {
                core.set_extension_status(&id, text);
            }
        })
    }

    /// The answer of the extension `package` to `call` for the project at `root`, noting why it could not answer when it could not.
    fn ask(
        &self,
        package: &Package,
        call: &HookCall,
        root: Option<&Path>,
    ) -> Result<HookReply, String> {
        let answered = pm_plugin::run_hook(
            package,
            call,
            root,
            &grants_of(&package.id),
            Some(self.status_of(&package.id)),
        );
        if let (Err(reason), Some(core)) = (&answered, self.core.upgrade()) {
            core.note_problem_package(&package.id, Some(reason.clone()));
            core.tell_extensions();
        }
        answered
    }

    /// The extension that brought `language`, when its code answers hooks.
    fn owner(language: &str) -> Option<Package> {
        language_owners()
            .remove(language)
            .filter(pm_plugin::answers_hooks)
    }

    /// The folder of the project as the extension is told of it: only when it may read the project.
    fn shown(package: &Package, root: &Path) -> Option<String> {
        grants_of(&package.id)
            .allows(Capability::ReadProject)
            .then(|| root.to_string_lossy().into_owned())
    }
}

/// The word a completion's kind is told to an extension as.
fn kind_word(kind: pm_lsp::CompletionKind) -> &'static str {
    match kind {
        pm_lsp::CompletionKind::Function => "function",
        pm_lsp::CompletionKind::Variable => "variable",
        pm_lsp::CompletionKind::Type => "type",
        pm_lsp::CompletionKind::Module => "module",
        pm_lsp::CompletionKind::Keyword => "keyword",
        pm_lsp::CompletionKind::Constant => "constant",
        pm_lsp::CompletionKind::Snippet => "snippet",
        pm_lsp::CompletionKind::Other => "other",
    }
}

impl Provider for Extensions {
    fn provide(
        &self,
        language: &str,
        spec: &ServerSpec,
        root: &Path,
        command: bool,
    ) -> Result<Provided, String> {
        let Some(package) = Self::owner(language) else {
            return Ok(Provided::default());
        };
        let project = Self::shown(&package, root);
        let mut provided = Provided::default();
        if command {
            grants_of(&package.id)
                .check(Capability::Process)
                .map_err(|denied| denied.to_string())?;
            let call = HookCall::LanguageServerCommand {
                server: spec.id.clone(),
                language: language.to_owned(),
                project: project.clone(),
                platform: pm_lsp::platform(),
            };
            if let HookReply::Command(held) = self.ask(&package, &call, Some(root))? {
                provided.command = Some(ProvidedCommand {
                    command: held.command,
                    args: held.args,
                    env: pm_task::safe_env(held.env),
                });
            }
        }
        let options = HookCall::LanguageServerInitializationOptions {
            server: spec.id.clone(),
            language: language.to_owned(),
            project: project.clone(),
        };
        if let HookReply::Json(value) = self.ask(&package, &options, Some(root))? {
            provided.init = Some(value);
        }
        let settings = HookCall::LanguageServerWorkspaceConfiguration {
            server: spec.id.clone(),
            language: language.to_owned(),
            project,
        };
        if let HookReply::Json(value) = self.ask(&package, &settings, Some(root))? {
            provided.settings = Some(value);
        }
        Ok(provided)
    }

    fn label_completions(
        &self,
        language: &str,
        spec: &ServerSpec,
        items: &[Completion],
    ) -> Vec<Option<String>> {
        let Some(package) = Self::owner(language) else {
            return vec![None; items.len()];
        };
        let call = HookCall::LabelForCompletions {
            server: spec.id.clone(),
            items: items
                .iter()
                .map(|item| CompletionItem {
                    label: item.label.clone(),
                    kind: kind_word(item.kind).to_owned(),
                    detail: item.detail.clone(),
                })
                .collect(),
        };
        match self.ask(&package, &call, None) {
            Ok(HookReply::Labels(labels)) if labels.len() == items.len() => labels,
            _ => vec![None; items.len()],
        }
    }

    fn label_symbols(
        &self,
        language: &str,
        spec: &ServerSpec,
        items: &[SymbolLabel],
    ) -> Vec<Option<String>> {
        let Some(package) = Self::owner(language) else {
            return vec![None; items.len()];
        };
        let call = HookCall::LabelForSymbols {
            server: spec.id.clone(),
            items: items
                .iter()
                .map(|item| SymbolItem {
                    name: item.name.clone(),
                    kind: item.kind.clone(),
                    container: item.container.clone(),
                })
                .collect(),
        };
        match self.ask(&package, &call, None) {
            Ok(HookReply::Labels(labels)) if labels.len() == items.len() => labels,
            _ => vec![None; items.len()],
        }
    }
}

impl pm_dap::BinaryProvider for Extensions {
    fn binary(&self, adapter: &str, install: bool) -> Result<Option<pm_dap::Binary>, String> {
        let package = pm_plugin::installed()
            .into_iter()
            .find(|package| {
                package
                    .manifest
                    .debug_adapters
                    .iter()
                    .any(|entry| entry.id == adapter && entry.provided)
            })
            .filter(pm_plugin::answers_hooks)
            .ok_or_else(|| format!("no extension provides {adapter}"))?;
        grants_of(&package.id)
            .check(Capability::Process)
            .map_err(|denied| denied.to_string())?;
        let call = HookCall::DebugAdapterBinary {
            adapter: adapter.to_owned(),
            install,
            platform: pm_lsp::platform(),
        };
        match self.ask(&package, &call, None)? {
            HookReply::Command(held) => Ok(Some(pm_dap::Binary {
                program: held.command.into(),
                arguments: held.args,
                env: pm_task::safe_env(held.env),
            })),
            _ => Ok(None),
        }
    }
}

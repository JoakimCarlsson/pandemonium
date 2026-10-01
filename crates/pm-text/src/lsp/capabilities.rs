//! What the editor tells a server it can do, and what a server says it does.
//!
//! A server says what it offers twice over: once in its answer to the
//! handshake, and again whenever it likes after that, by registering a method
//! for the documents a selector picks out. Both are kept here, and every
//! question of whether a server answers a method about a document is asked
//! of both at once, so no caller has to know which way the server chose.

use std::collections::HashMap;
use std::path::Path;

use globset::GlobBuilder;
use lsp_types::{
    CallHierarchyClientCapabilities, CallHierarchyServerCapability, ClientCapabilities, ClientInfo,
    CodeActionClientCapabilities, CodeActionKind, CodeActionKindLiteralSupport,
    CodeActionLiteralSupport, CodeActionOptions, CodeActionProviderCapability,
    CodeLensClientCapabilities, CodeLensOptions, CodeLensWorkspaceClientCapabilities,
    CompletionClientCapabilities, CompletionItemCapability, CompletionItemCapabilityResolveSupport,
    CompletionOptions, DeclarationCapability, DiagnosticClientCapabilities, DiagnosticOptions,
    DiagnosticServerCapabilities, DiagnosticWorkspaceClientCapabilities,
    DidChangeWatchedFilesClientCapabilities, DocumentFilter, DocumentFormattingClientCapabilities,
    DocumentHighlightClientCapabilities, DocumentSelector, DocumentSymbolClientCapabilities,
    DynamicRegistrationClientCapabilities, GeneralClientCapabilities, GotoCapability,
    HoverClientCapabilities, HoverProviderCapability, ImplementationProviderCapability,
    InitializeParams, InlayHintClientCapabilities, InlayHintWorkspaceClientCapabilities,
    InlineCompletionClientCapabilities, MarkupKind, OneOf, PositionEncodingKind,
    PublishDiagnosticsClientCapabilities, ReferenceClientCapabilities, Registration,
    RenameClientCapabilities, ResourceOperationKind, SaveOptions, SemanticTokenModifier,
    SemanticTokenType, SemanticTokensClientCapabilities, SemanticTokensClientCapabilitiesRequests,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensServerCapabilities,
    SemanticTokensWorkspaceClientCapabilities, ServerCapabilities, SignatureHelpClientCapabilities,
    SignatureHelpOptions, TextDocumentChangeRegistrationOptions, TextDocumentClientCapabilities,
    TextDocumentRegistrationOptions, TextDocumentSaveRegistrationOptions,
    TextDocumentSyncCapability, TextDocumentSyncClientCapabilities, TextDocumentSyncKind,
    TextDocumentSyncSaveOptions, TokenFormat, TypeDefinitionProviderCapability,
    WindowClientCapabilities, WorkspaceClientCapabilities, WorkspaceEditClientCapabilities,
    WorkspaceFolder, WorkspaceSymbolClientCapabilities,
};
use lsp_types::{
    DocumentOnTypeFormattingClientCapabilities, DocumentOnTypeFormattingOptions,
    DocumentRangeFormattingClientCapabilities, FileOperationFilter, FileOperationPatternKind,
    FileOperationRegistrationOptions, FoldingRangeClientCapabilities,
    FoldingRangeProviderCapability, ParameterInformationSettings, RenameOptions,
    SelectionRangeClientCapabilities, SelectionRangeProviderCapability,
    SignatureInformationSettings, WorkspaceFileOperationsClientCapabilities,
};
use serde_json::Value;

use crate::language::Server;
use crate::lsp::uri;

/// The semantic token types the editor understands, in the protocol's words.
///
/// A server hands back a legend of its own, in its own order, of the types
/// it will use out of these; a type the editor did not ask for is one it
/// will not be sent.
const TOKEN_TYPES: [&str; 23] = [
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "macro",
    "keyword",
    "modifier",
    "comment",
    "string",
    "number",
    "regexp",
    "operator",
    "decorator",
];

/// The code action kinds the editor offers in its menu.
const ACTION_KINDS: [&str; 8] = [
    "quickfix",
    "refactor",
    "refactor.extract",
    "refactor.inline",
    "refactor.rewrite",
    "source",
    "source.organizeImports",
    "source.fixAll",
];

/// The method a registration of code actions names.
const CODE_ACTION: &str = "textDocument/codeAction";

/// The parts of a completion a server may leave out of the list and send
/// only when the item is resolved.
const RESOLVED_COMPLETION: [&str; 3] = ["documentation", "detail", "additionalTextEdits"];

/// The method a registration of document synchronisation's changes names.
const DID_CHANGE: &str = "textDocument/didChange";

/// The method a registration of saving names.
const DID_SAVE: &str = "textDocument/didSave";

/// The method a registration of semantic tokens names.
const SEMANTIC_TOKENS: &str = "textDocument/semanticTokens";

/// The method a registration of completion names.
const COMPLETION: &str = "textDocument/completion";

/// The method a registration of signature help names.
const SIGNATURE_HELP: &str = "textDocument/signatureHelp";

/// The method a registration of code lenses names.
const CODE_LENS: &str = "textDocument/codeLens";

/// The method a registration of renaming names.
const RENAME: &str = "textDocument/rename";

/// The method a registration of formatting as the reader types names.
const ON_TYPE: &str = "textDocument/onTypeFormatting";

/// The method a registration of pulled diagnostics names.
const DIAGNOSTIC: &str = "textDocument/diagnostic";

/// One open document, as a server's selectors pick documents out.
#[derive(Clone, Copy)]
pub(super) struct Document<'a> {
    /// Where it lives.
    pub path: &'a Path,
    /// The language it was opened as, in the protocol's words.
    pub language: Option<&'a str>,
}

/// One method a server registered after the handshake.
struct Registered {
    /// The method it registered.
    method: String,
    /// The documents it registered it for, or every document when absent.
    selector: Option<DocumentSelector>,
    /// What it registered it with, read by whichever method it is.
    options: Value,
}

/// Everything a server has said it can do.
#[derive(Default)]
pub(super) struct Capabilities {
    /// What it said in its answer to the handshake, once it has answered.
    stated: Option<ServerCapabilities>,
    /// What it registered since, by the id it registered each under.
    registered: HashMap<String, Registered>,
}

impl Capabilities {
    /// Whether the handshake has been answered and the server has said.
    pub(super) fn is_known(&self) -> bool {
        self.stated.is_some()
    }

    /// Takes in what the handshake's answer said.
    pub(super) fn state(&mut self, capabilities: ServerCapabilities) {
        self.stated = Some(capabilities);
    }

    /// Takes in the registrations a server sent.
    pub(super) fn register(&mut self, registrations: Vec<Registration>) {
        for registration in registrations {
            let options = registration.register_options.unwrap_or(Value::Null);
            let selector =
                serde_json::from_value::<TextDocumentRegistrationOptions>(options.clone())
                    .ok()
                    .and_then(|options| options.document_selector);
            self.registered.insert(
                registration.id,
                Registered {
                    method: registration.method,
                    selector,
                    options,
                },
            );
        }
    }

    /// Forgets the registrations whose ids are in `ids`.
    pub(super) fn unregister(&mut self, ids: impl IntoIterator<Item = String>) {
        for id in ids {
            self.registered.remove(&id);
        }
    }

    /// The options every registration of `method` that picks out `document`
    /// was made with.
    fn registrations<'a>(
        &'a self,
        method: &'a str,
        document: Option<Document<'a>>,
    ) -> impl Iterator<Item = &'a Value> + 'a {
        self.registered
            .values()
            .filter(move |registered| registered.method == method)
            .filter(move |registered| match (&registered.selector, document) {
                (Some(selector), Some(document)) => selects(selector, document),
                _ => true,
            })
            .map(|registered| &registered.options)
    }

    /// Whether any registration of `method` picks out `document`.
    fn has_registered(&self, method: &str, document: Option<Document>) -> bool {
        self.registrations(method, document).next().is_some()
    }

    /// Whether the server answers `method` about `document`, or at all when
    /// no document is named.
    ///
    /// A server that has not answered the handshake yet has not said, and is
    /// taken to answer everything: what is asked before the handshake waits
    /// for it, and is refused then if the server turns out not to.
    pub(super) fn offers(&self, method: &str, document: Option<Document>) -> bool {
        let Some(stated) = self.stated.as_ref() else {
            return true;
        };
        states(stated, method) || self.has_registered(method, document)
    }

    /// How the server wants to hear about changes to `document`.
    pub(super) fn sync(&self, document: Document) -> TextDocumentSyncKind {
        let registered = self
            .registrations(DID_CHANGE, Some(document))
            .filter_map(|options| {
                serde_json::from_value::<TextDocumentChangeRegistrationOptions>(options.clone())
                    .ok()
            })
            .map(|options| options.sync_kind)
            .max_by_key(sync_rank);
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.text_document_sync.as_ref())
            .map(|sync| match sync {
                TextDocumentSyncCapability::Kind(kind) => *kind,
                TextDocumentSyncCapability::Options(options) => {
                    options.change.unwrap_or(TextDocumentSyncKind::NONE)
                }
            });
        [registered, stated]
            .into_iter()
            .flatten()
            .max_by_key(sync_rank)
            .unwrap_or(TextDocumentSyncKind::NONE)
    }

    /// Whether the server wants to hear that `document` is about to be saved.
    pub(super) fn wants_will_save(&self, document: Document) -> bool {
        self.sync_options()
            .is_some_and(|options| options.will_save == Some(true))
            || self.has_registered("textDocument/willSave", Some(document))
    }

    /// Whether the server wants to hear that `document` was saved, and whether
    /// with its text.
    pub(super) fn wants_saved(&self, document: Document) -> Option<bool> {
        let registered = self
            .registrations(DID_SAVE, Some(document))
            .map(|options| {
                serde_json::from_value::<TextDocumentSaveRegistrationOptions>(options.clone())
                    .ok()
                    .and_then(|options| options.include_text)
                    .unwrap_or(false)
            })
            .reduce(|either, or| either || or);
        let stated = match self.stated.as_ref()?.text_document_sync.as_ref()? {
            TextDocumentSyncCapability::Kind(_) => Some(false),
            TextDocumentSyncCapability::Options(options) => match options.save.as_ref() {
                None | Some(TextDocumentSyncSaveOptions::Supported(false)) => None,
                Some(TextDocumentSyncSaveOptions::Supported(true)) => Some(false),
                Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions { include_text })) => {
                    Some(include_text.unwrap_or(false))
                }
            },
        };
        [registered, stated]
            .into_iter()
            .flatten()
            .reduce(|either, or| either || or)
    }

    /// The server's document synchronisation, when it stated it as options.
    fn sync_options(&self) -> Option<&lsp_types::TextDocumentSyncOptions> {
        match self.stated.as_ref()?.text_document_sync.as_ref()? {
            TextDocumentSyncCapability::Options(options) => Some(options),
            TextDocumentSyncCapability::Kind(_) => None,
        }
    }

    /// Whether the server says it makes code actions of `kind`, or of a kind
    /// that contains it.
    ///
    /// A server that does not list the kinds it makes is taken to make none
    /// of the source kinds a save asks for: asking it would be asking for
    /// whatever it likes to answer with.
    pub(super) fn offers_action_kind(&self, kind: &str, document: Option<Document>) -> bool {
        let covers = |kinds: &[CodeActionKind]| {
            kinds.iter().any(|offered| {
                kind == offered.as_str()
                    || kind
                        .strip_prefix(offered.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            })
        };
        let stated = match self
            .stated
            .as_ref()
            .and_then(|stated| stated.code_action_provider.as_ref())
        {
            Some(CodeActionProviderCapability::Options(options)) => {
                options.code_action_kinds.as_deref().is_some_and(covers)
            }
            _ => false,
        };
        stated
            || self.registrations(CODE_ACTION, document).any(|options| {
                serde_json::from_value::<CodeActionOptions>(options.clone())
                    .is_ok_and(|options| options.code_action_kinds.as_deref().is_some_and(covers))
            })
    }

    /// Whether the server fills in completions it sent short, when asked.
    pub(super) fn resolves_completions(&self, document: Option<Document>) -> bool {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.completion_provider.as_ref())
            .is_some_and(|options| options.resolve_provider == Some(true));
        stated
            || self.registrations(COMPLETION, document).any(|options| {
                serde_json::from_value::<CompletionOptions>(options.clone())
                    .is_ok_and(|options| options.resolve_provider == Some(true))
            })
    }

    /// Whether the server wants to hear about the file operation `method` on
    /// `path`, by what its answer to the handshake or a registration says.
    ///
    /// A path that is no longer on disk matches a filter for files and one
    /// for folders alike, since what it was can no longer be asked.
    pub(super) fn file_operation(&self, method: &str, path: &Path) -> bool {
        let Some(stated) = self.stated.as_ref() else {
            return false;
        };
        let operations = stated
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.file_operations.as_ref());
        let stated = operations.and_then(|operations| match method {
            "workspace/didCreateFiles" => operations.did_create.clone(),
            "workspace/didRenameFiles" => operations.did_rename.clone(),
            "workspace/willRenameFiles" => operations.will_rename.clone(),
            "workspace/didDeleteFiles" => operations.did_delete.clone(),
            _ => None,
        });
        let registered = self.registrations(method, None).filter_map(|options| {
            serde_json::from_value::<FileOperationRegistrationOptions>(options.clone()).ok()
        });
        let folder = std::fs::metadata(path).ok().map(|found| found.is_dir());
        stated
            .into_iter()
            .chain(registered)
            .flat_map(|options| options.filters)
            .any(|filter| files(&filter, path, folder))
    }

    /// Whether the server says whether a place can be renamed before it is.
    pub(super) fn prepares_renames(&self, document: Option<Document>) -> bool {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.rename_provider.as_ref())
            .is_some_and(|provider| {
                matches!(provider, OneOf::Right(options) if options.prepare_provider == Some(true))
            });
        stated
            || self.registrations(RENAME, document).any(|options| {
                serde_json::from_value::<RenameOptions>(options.clone())
                    .is_ok_and(|options| options.prepare_provider == Some(true))
            })
    }

    /// The characters the server formats after, as soon as one is typed.
    pub(super) fn on_type_triggers(&self, document: Option<Document>) -> Vec<char> {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.document_on_type_formatting_provider.clone());
        let registered = self.registrations(ON_TYPE, document).filter_map(|options| {
            serde_json::from_value::<DocumentOnTypeFormattingOptions>(options.clone()).ok()
        });
        stated
            .into_iter()
            .chain(registered)
            .flat_map(|options| {
                std::iter::once(options.first_trigger_character)
                    .chain(options.more_trigger_character.unwrap_or_default())
            })
            .filter_map(|written| written.chars().next())
            .collect()
    }

    /// Whether the server answers what changed in a file's semantic tokens
    /// since it last sent them, rather than all of them again.
    pub(super) fn sends_semantic_deltas(&self) -> bool {
        let delta = |full: &Option<SemanticTokensFullOptions>| {
            matches!(
                full,
                Some(SemanticTokensFullOptions::Delta { delta: Some(true) })
            )
        };
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.semantic_tokens_provider.as_ref())
            .is_some_and(|provider| match provider {
                SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
                    delta(&options.full)
                }
                SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
                    delta(&options.semantic_tokens_options.full)
                }
            });
        stated
            || self.registrations(SEMANTIC_TOKENS, None).any(|options| {
                serde_json::from_value::<lsp_types::SemanticTokensOptions>(options.clone())
                    .is_ok_and(|options| delta(&options.full))
            })
    }

    /// The characters the server completes after, as soon as one is typed.
    pub(super) fn completion_triggers(&self, document: Option<Document>) -> Vec<char> {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.completion_provider.as_ref())
            .and_then(|options| options.trigger_characters.clone())
            .unwrap_or_default();
        let registered = self
            .registrations(COMPLETION, document)
            .filter_map(|options| serde_json::from_value::<CompletionOptions>(options.clone()).ok())
            .flat_map(|options| options.trigger_characters.unwrap_or_default());
        stated
            .into_iter()
            .chain(registered)
            .filter_map(|written| written.chars().next())
            .collect()
    }

    /// The characters that start or refresh signature help for `document`.
    pub(super) fn signature_triggers(
        &self,
        document: Option<Document>,
        showing: bool,
    ) -> Vec<char> {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.signature_help_provider.as_ref());
        let registered = self
            .registrations(SIGNATURE_HELP, document)
            .filter_map(|options| {
                serde_json::from_value::<SignatureHelpOptions>(options.clone()).ok()
            });
        stated
            .cloned()
            .into_iter()
            .chain(registered)
            .flat_map(|options| {
                options.trigger_characters.into_iter().flatten().chain(
                    options
                        .retrigger_characters
                        .into_iter()
                        .flatten()
                        .filter(|_| showing),
                )
            })
            .filter_map(|written| written.chars().next())
            .collect()
    }

    /// Whether the server says what a code lens it sent unsaid says, when asked.
    pub(super) fn resolves_lenses(&self, document: Option<Document>) -> bool {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.code_lens_provider.as_ref())
            .is_some_and(|options| options.resolve_provider == Some(true));
        stated
            || self.registrations(CODE_LENS, document).any(|options| {
                serde_json::from_value::<CodeLensOptions>(options.clone())
                    .is_ok_and(|options| options.resolve_provider == Some(true))
            })
    }

    /// How the server asks for its diagnostics to be pulled for `document`,
    /// when it does.
    pub(super) fn pulls(&self, document: Document) -> Option<DiagnosticOptions> {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.diagnostic_provider.as_ref())
            .map(|provider| match provider {
                DiagnosticServerCapabilities::Options(options) => options.clone(),
                DiagnosticServerCapabilities::RegistrationOptions(options) => {
                    options.diagnostic_options.clone()
                }
            });
        stated.or_else(|| {
            self.registrations(DIAGNOSTIC, Some(document))
                .find_map(|options| {
                    serde_json::from_value::<DiagnosticOptions>(options.clone()).ok()
                })
        })
    }

    /// What the server said its semantic token types are, in its own order.
    pub(super) fn legend(&self) -> Option<SemanticTokensLegend> {
        let stated = self
            .stated
            .as_ref()
            .and_then(|stated| stated.semantic_tokens_provider.as_ref())
            .map(|provider| match provider {
                SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
                    options.legend.clone()
                }
                SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
                    options.semantic_tokens_options.legend.clone()
                }
            });
        stated.or_else(|| {
            self.registrations(SEMANTIC_TOKENS, None)
                .find_map(|options| {
                    serde_json::from_value::<lsp_types::SemanticTokensOptions>(options.clone())
                        .ok()
                        .map(|options| options.legend)
                })
        })
    }
}

/// How much of a document a kind of synchronisation sends, the least first.
fn sync_rank(kind: &TextDocumentSyncKind) -> u8 {
    match *kind {
        TextDocumentSyncKind::INCREMENTAL => 2,
        TextDocumentSyncKind::FULL => 1,
        _ => 0,
    }
}

/// Whether the file operation filter `filter` takes in `path`, which is a
/// folder when `folder` says so and either when it cannot be told.
fn files(filter: &FileOperationFilter, path: &Path, folder: Option<bool>) -> bool {
    let scheme = filter
        .scheme
        .as_deref()
        .is_none_or(|scheme| scheme == "file");
    let kind = !matches!(
        (&filter.pattern.matches, folder),
        (Some(FileOperationPatternKind::File), Some(true))
            | (Some(FileOperationPatternKind::Folder), Some(false))
    );
    let ignore_case = filter
        .pattern
        .options
        .as_ref()
        .and_then(|options| options.ignore_case)
        .unwrap_or(false);
    let glob = GlobBuilder::new(&filter.pattern.glob)
        .literal_separator(true)
        .case_insensitive(ignore_case)
        .build()
        .is_ok_and(|glob| glob.compile_matcher().is_match(path));
    scheme && kind && glob
}

/// Whether any filter of `selector` picks out `document`.
fn selects(selector: &DocumentSelector, document: Document) -> bool {
    selector.iter().any(|filter| picks(filter, document))
}

/// Whether `filter` picks out `document`: every part it names must match.
fn picks(filter: &DocumentFilter, document: Document) -> bool {
    let language = filter
        .language
        .as_deref()
        .is_none_or(|language| document.language == Some(language));
    let scheme = filter
        .scheme
        .as_deref()
        .is_none_or(|scheme| scheme == "file");
    let pattern = filter.pattern.as_deref().is_none_or(|pattern| {
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .is_ok_and(|glob| glob.compile_matcher().is_match(document.path))
    });
    language && scheme && pattern
}

/// Whether a provider the handshake's answer carries says it answers.
trait Provides {
    /// Whether it does.
    fn provides(&self) -> bool;
}

impl<T> Provides for OneOf<bool, T> {
    fn provides(&self) -> bool {
        !matches!(self, OneOf::Left(false))
    }
}

impl Provides for HoverProviderCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for TypeDefinitionProviderCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for ImplementationProviderCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for CodeActionProviderCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for DeclarationCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for FoldingRangeProviderCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for SelectionRangeProviderCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

impl Provides for CallHierarchyServerCapability {
    fn provides(&self) -> bool {
        !matches!(self, Self::Simple(false))
    }
}

/// Whether `provider` is there and says it answers.
fn on(provider: Option<&impl Provides>) -> bool {
    provider.is_some_and(Provides::provides)
}

/// Whether the handshake's answer `stated` says the server answers `method`.
fn states(stated: &ServerCapabilities, method: &str) -> bool {
    match method {
        "textDocument/definition" => on(stated.definition_provider.as_ref()),
        "textDocument/typeDefinition" => on(stated.type_definition_provider.as_ref()),
        "textDocument/implementation" => on(stated.implementation_provider.as_ref()),
        "textDocument/declaration" => on(stated.declaration_provider.as_ref()),
        "textDocument/references" => on(stated.references_provider.as_ref()),
        "textDocument/hover" => on(stated.hover_provider.as_ref()),
        "textDocument/completion" => stated.completion_provider.is_some(),
        "textDocument/inlineCompletion" => on(stated.inline_completion_provider.as_ref()),
        "textDocument/signatureHelp" => stated.signature_help_provider.is_some(),
        "textDocument/codeAction" => on(stated.code_action_provider.as_ref()),
        "textDocument/rename" => on(stated.rename_provider.as_ref()),
        "textDocument/formatting" => on(stated.document_formatting_provider.as_ref()),
        "textDocument/rangeFormatting" => on(stated.document_range_formatting_provider.as_ref()),
        "textDocument/onTypeFormatting" => stated.document_on_type_formatting_provider.is_some(),
        "textDocument/foldingRange" => on(stated.folding_range_provider.as_ref()),
        "textDocument/selectionRange" => on(stated.selection_range_provider.as_ref()),
        "textDocument/documentSymbol" => on(stated.document_symbol_provider.as_ref()),
        "textDocument/inlayHint" => on(stated.inlay_hint_provider.as_ref()),
        "textDocument/semanticTokens" => {
            stated
                .semantic_tokens_provider
                .as_ref()
                .is_some_and(|provider| {
                    let full = match provider {
                        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
                            &options.full
                        }
                        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                            options,
                        ) => &options.semantic_tokens_options.full,
                    };
                    !matches!(full, None | Some(SemanticTokensFullOptions::Bool(false)))
                })
        }
        "textDocument/documentHighlight" => on(stated.document_highlight_provider.as_ref()),
        "textDocument/codeLens" => stated.code_lens_provider.is_some(),
        "textDocument/prepareCallHierarchy" => on(stated.call_hierarchy_provider.as_ref()),
        "workspace/symbol" => on(stated.workspace_symbol_provider.as_ref()),
        "textDocument/willSaveWaitUntil" => match stated.text_document_sync.as_ref() {
            Some(TextDocumentSyncCapability::Options(options)) => {
                options.will_save_wait_until == Some(true)
            }
            _ => false,
        },
        "textDocument/diagnostic" => stated.diagnostic_provider.is_some(),
        _ => false,
    }
}

/// Everything the editor says about itself when it starts `server` over `root`.
pub(super) fn initialize(root: &Path, server: Server) -> InitializeParams {
    let options = serde_json::from_str::<Value>(server.options).ok();
    #[allow(deprecated)]
    InitializeParams {
        process_id: Some(std::process::id()),
        root_uri: Some(uri::typed(root)),
        initialization_options: options,
        capabilities: client(),
        workspace_folders: Some(vec![folder(root)]),
        client_info: Some(ClientInfo {
            name: "Pandemonium".to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        ..InitializeParams::default()
    }
}

/// The one workspace folder a server over `root` is started for.
pub(super) fn folder(root: &Path) -> WorkspaceFolder {
    WorkspaceFolder {
        uri: uri::typed(root),
        name: root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    }
}

/// A capability that may be registered after the handshake.
fn dynamic() -> Option<bool> {
    Some(true)
}

/// A go-to kind of question, answered with links as well as locations.
fn goto() -> Option<GotoCapability> {
    Some(GotoCapability {
        dynamic_registration: dynamic(),
        link_support: Some(true),
    })
}

/// What the editor can do, in the protocol's words.
fn client() -> ClientCapabilities {
    ClientCapabilities {
        text_document: Some(text_document()),
        workspace: Some(workspace()),
        window: Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            show_message: None,
            show_document: None,
        }),
        general: Some(GeneralClientCapabilities {
            position_encodings: Some(vec![
                PositionEncodingKind::UTF32,
                PositionEncodingKind::UTF8,
                PositionEncodingKind::UTF16,
            ]),
            ..GeneralClientCapabilities::default()
        }),
        ..ClientCapabilities::default()
    }
}

/// What the editor can do with one document, in the protocol's words.
fn text_document() -> TextDocumentClientCapabilities {
    TextDocumentClientCapabilities {
        synchronization: Some(TextDocumentSyncClientCapabilities {
            dynamic_registration: dynamic(),
            will_save: Some(true),
            will_save_wait_until: Some(true),
            did_save: Some(true),
        }),
        completion: Some(CompletionClientCapabilities {
            dynamic_registration: dynamic(),
            completion_item: Some(CompletionItemCapability {
                snippet_support: Some(true),
                documentation_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                insert_replace_support: Some(true),
                label_details_support: Some(true),
                commit_characters_support: Some(true),
                deprecated_support: Some(true),
                preselect_support: Some(true),
                tag_support: Some(lsp_types::TagSupport {
                    value_set: vec![lsp_types::CompletionItemTag::DEPRECATED],
                }),
                resolve_support: Some(CompletionItemCapabilityResolveSupport {
                    properties: RESOLVED_COMPLETION.map(str::to_owned).to_vec(),
                }),
                ..CompletionItemCapability::default()
            }),
            context_support: Some(true),
            ..CompletionClientCapabilities::default()
        }),
        inline_completion: Some(InlineCompletionClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        hover: Some(HoverClientCapabilities {
            dynamic_registration: dynamic(),
            content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
        }),
        signature_help: Some(SignatureHelpClientCapabilities {
            dynamic_registration: dynamic(),
            signature_information: Some(SignatureInformationSettings {
                documentation_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                parameter_information: Some(ParameterInformationSettings {
                    label_offset_support: Some(true),
                }),
                active_parameter_support: Some(true),
            }),
            ..SignatureHelpClientCapabilities::default()
        }),
        references: Some(ReferenceClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        document_highlight: Some(DocumentHighlightClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        document_symbol: Some(DocumentSymbolClientCapabilities {
            dynamic_registration: dynamic(),
            hierarchical_document_symbol_support: Some(true),
            ..DocumentSymbolClientCapabilities::default()
        }),
        formatting: Some(DocumentFormattingClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        range_formatting: Some(DocumentRangeFormattingClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        on_type_formatting: Some(DocumentOnTypeFormattingClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        folding_range: Some(FoldingRangeClientCapabilities {
            dynamic_registration: dynamic(),
            line_folding_only: Some(true),
            ..FoldingRangeClientCapabilities::default()
        }),
        selection_range: Some(SelectionRangeClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        declaration: goto(),
        definition: goto(),
        type_definition: goto(),
        implementation: goto(),
        code_action: Some(CodeActionClientCapabilities {
            dynamic_registration: dynamic(),
            code_action_literal_support: Some(CodeActionLiteralSupport {
                code_action_kind: CodeActionKindLiteralSupport {
                    value_set: ACTION_KINDS.map(str::to_owned).to_vec(),
                },
            }),
            ..CodeActionClientCapabilities::default()
        }),
        code_lens: Some(CodeLensClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        rename: Some(RenameClientCapabilities {
            dynamic_registration: dynamic(),
            prepare_support: Some(true),
            ..RenameClientCapabilities::default()
        }),
        publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
            related_information: Some(false),
            tag_support: Some(lsp_types::TagSupport {
                value_set: vec![lsp_types::DiagnosticTag::UNNECESSARY],
            }),
            ..PublishDiagnosticsClientCapabilities::default()
        }),
        call_hierarchy: Some(CallHierarchyClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        semantic_tokens: Some(SemanticTokensClientCapabilities {
            dynamic_registration: dynamic(),
            requests: SemanticTokensClientCapabilitiesRequests {
                range: Some(false),
                full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
            },
            token_types: TOKEN_TYPES.map(SemanticTokenType::new).to_vec(),
            token_modifiers: vec![SemanticTokenModifier::new("mutable")],
            formats: vec![TokenFormat::RELATIVE],
            overlapping_token_support: None,
            multiline_token_support: None,
            server_cancel_support: Some(true),
            augments_syntax_tokens: Some(true),
        }),
        inlay_hint: Some(InlayHintClientCapabilities {
            dynamic_registration: dynamic(),
            resolve_support: None,
        }),
        diagnostic: Some(DiagnosticClientCapabilities {
            dynamic_registration: dynamic(),
            related_document_support: Some(true),
        }),
        ..TextDocumentClientCapabilities::default()
    }
}

/// What the editor can do across a workspace, in the protocol's words.
fn workspace() -> WorkspaceClientCapabilities {
    WorkspaceClientCapabilities {
        apply_edit: Some(true),
        workspace_edit: Some(WorkspaceEditClientCapabilities {
            document_changes: Some(true),
            resource_operations: Some(vec![
                ResourceOperationKind::Create,
                ResourceOperationKind::Rename,
                ResourceOperationKind::Delete,
            ]),
            ..WorkspaceEditClientCapabilities::default()
        }),
        did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
            dynamic_registration: dynamic(),
            relative_pattern_support: Some(true),
        }),
        symbol: Some(WorkspaceSymbolClientCapabilities {
            dynamic_registration: dynamic(),
            ..WorkspaceSymbolClientCapabilities::default()
        }),
        execute_command: Some(DynamicRegistrationClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        workspace_folders: Some(true),
        configuration: Some(true),
        did_change_configuration: Some(DynamicRegistrationClientCapabilities {
            dynamic_registration: dynamic(),
        }),
        file_operations: Some(WorkspaceFileOperationsClientCapabilities {
            dynamic_registration: dynamic(),
            did_create: Some(true),
            will_create: Some(false),
            did_rename: Some(true),
            will_rename: Some(true),
            did_delete: Some(true),
            will_delete: Some(false),
        }),
        semantic_tokens: Some(SemanticTokensWorkspaceClientCapabilities {
            refresh_support: Some(true),
        }),
        code_lens: Some(CodeLensWorkspaceClientCapabilities {
            refresh_support: Some(true),
        }),
        inlay_hint: Some(InlayHintWorkspaceClientCapabilities {
            refresh_support: Some(true),
        }),
        diagnostic: Some(DiagnosticWorkspaceClientCapabilities {
            refresh_support: Some(true),
        }),
        ..WorkspaceClientCapabilities::default()
    }
}

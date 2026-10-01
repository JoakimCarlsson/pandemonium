//! What a language server can be asked, and what it says back.
//!
//! The protocol's own shapes stop here. A request is one of a closed set of
//! questions, an answer is one of a closed set of replies, and both are in
//! the editor's own terms — paths and [`Position`]s — so nothing above this
//! layer ever sees a `file://` URI or a JSON value. Below it everything is
//! the protocol's own type for the method being asked: each question is
//! built as that method's params and each reply read as that method's result.

use std::ops::Range;
use std::path::{Path, PathBuf};

use lsp_types::request::{
    CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
    CodeActionRequest, CodeLensRequest, CodeLensResolve, Completion as CompletionRequest,
    DocumentHighlightRequest, DocumentSymbolRequest, FoldingRangeRequest, Formatting,
    GotoDeclaration, GotoDefinition, GotoImplementation, GotoTypeDefinition, HoverRequest,
    InlayHintRequest, InlineCompletionRequest, OnTypeFormatting, PrepareRenameRequest,
    RangeFormatting, References, Rename, ResolveCompletionItem, SelectionRangeRequest,
    SemanticTokensFullRequest, SignatureHelpRequest, WillRenameFiles, WillSaveWaitUntil,
    WorkspaceSymbolRequest,
};
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeLens,
    CodeLensParams, CompletionContext, CompletionItem, CompletionItemKind, CompletionItemTag,
    CompletionParams, CompletionResponse, CompletionTextEdit, CompletionTriggerKind,
    DocumentChangeOperation, DocumentChanges, DocumentFormattingParams, DocumentHighlightParams,
    DocumentOnTypeFormattingParams, DocumentRangeFormattingParams, DocumentSymbol,
    DocumentSymbolParams, DocumentSymbolResponse, Documentation, FileRename, FoldingRangeParams,
    FormattingOptions, GotoDefinitionParams, GotoDefinitionResponse, HoverContents, HoverParams,
    InlayHint, InlayHintLabel, InlayHintParams, InlineCompletionContext, InlineCompletionParams,
    InlineCompletionResponse, InlineCompletionTriggerKind, InsertTextFormat, MarkedString, OneOf,
    ParameterLabel, PartialResultParams, PrepareRenameResponse, ReferenceContext, ReferenceParams,
    RenameFilesParams, RenameParams, ResourceOp, SelectionRangeParams, SemanticToken,
    SemanticTokensParams, SemanticTokensResult, SignatureHelp, SignatureHelpParams,
    SymbolInformation, SymbolKind, TextDocumentIdentifier, TextDocumentPositionParams,
    TextDocumentSaveReason, TextEdit, WillSaveTextDocumentParams, WorkDoneProgressParams,
    WorkspaceEdit, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use serde_json::Value;

use crate::cursor::Position;
use crate::indent::Indent;
use crate::lsp::capabilities::{Capabilities, Document};
use crate::lsp::encoding::Files;
use crate::lsp::{rpc, uri};
use crate::predict::Prediction;
use crate::syntax::Highlight;

/// One thing a language server can be asked about a place in a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    /// Where the symbol here is defined.
    Definition,
    /// Where the type of the symbol here is defined.
    TypeDefinition,
    /// What implements the symbol here.
    Implementation,
    /// Where the symbol here is declared.
    Declaration,
    /// Everywhere the symbol here is used.
    References,
    /// What the server has to say about the symbol here.
    Hover,
    /// What could be written here, and what made the editor ask.
    Completions(Trigger),
    /// Text predicted after the cursor from its surrounding code.
    InlineCompletion,
    /// The rest of what one of those completions says, for a server that
    /// sent it short.
    ResolveCompletion(Handle),
    /// The signature of the call this place is inside.
    Signature,
    /// The fixes and refactors the server offers here.
    CodeActions,
    /// The changes the server would make to the whole file for one kind of
    /// source action, such as organizing its imports, named by the kind.
    SourceActions(String),
    /// Rename the symbol here to this everywhere it appears.
    Rename(String),
    /// Lay the whole file out the way the server's formatter would.
    Format,
    /// Lay the selected lines out the way the server's formatter would.
    FormatSelection,
    /// What the server would change now that this character was typed.
    FormatOnType(char),
    /// Whether the symbol here can be renamed, and what it is called.
    PrepareRename,
    /// The runs of lines the server says fold together.
    Folds,
    /// The spans around the cursor, from the smallest out, that the
    /// selection can grow through.
    SelectionRanges,
    /// The symbols the file declares.
    Symbols,
    /// What the server would write into the lines of this span.
    Hints(Range<Position>),
    /// What the server makes of every name in the file.
    Semantics,
    /// Every place in the file the symbol here is read or written.
    Occurrences,
    /// The notes the server would put above the file's declarations.
    Lenses,
    /// What one of those notes says, for a server that sent it unsaid.
    ResolveLens(Handle),
    /// The symbol here, as the server will follow its calls one way.
    PrepareCalls(Calls),
    /// The calls one way along from a symbol the server named.
    Calls(Calls, Handle),
    /// The symbols of the whole workspace whose names match this.
    WorkspaceSymbols(String),
    /// What the server would change in the file before it is saved.
    WillSave,
    /// What the server would change elsewhere before these files and
    /// folders are moved: the imports and declarations that name them.
    WillRenameFiles(Vec<(PathBuf, PathBuf)>),
}

/// What made the editor ask what could be written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trigger {
    /// The reader asked, or typed a word.
    Invoked,
    /// The reader typed a character a server said it completes after.
    Character(char),
    /// The reader typed on while a server's last list said it was not all
    /// there was.
    Incomplete,
}

/// Which way along a symbol's calls to follow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Calls {
    /// Whatever calls the symbol.
    Incoming,
    /// Whatever the symbol calls.
    Outgoing,
}

/// Something a server handed out to be handed back to it as it was.
///
/// A code lens to resolve, a completion to fill in and a symbol whose calls
/// are to be followed are all the server's own records: the editor keeps
/// them without reading them and returns them to the server that made them.
#[derive(Clone, Debug, PartialEq)]
pub struct Handle(Handed);

impl Eq for Handle {}

/// What a server handed out, by the question it will be handed back with.
#[derive(Clone, Debug, PartialEq)]
enum Handed {
    /// A code lens, to be resolved.
    Lens(Box<CodeLens>),
    /// A symbol, whose calls are to be followed.
    Call(Box<CallHierarchyItem>),
    /// A completion, to be filled in.
    Completion(Box<CompletionItem>),
    /// A word of the file, which no server made and none can fill in.
    Word,
}

/// Where a question is asked, already counted the server's way.
pub(super) struct Asking<'a> {
    /// The file it is about.
    pub path: &'a Path,
    /// The place in it.
    pub at: Position,
    /// The span selected in it, or the line the place is on.
    pub selection: Range<Position>,
    /// How the file is indented, for a formatter.
    pub indent: Indent,
    /// What the server said is wrong across the selection, for its fixes.
    pub diagnostics: Vec<lsp_types::Diagnostic>,
}

impl Request {
    /// The method a server registers to answer this, and states it answers
    /// in its answer to the handshake.
    pub(super) fn capability(&self) -> &'static str {
        match self {
            Self::Definition => "textDocument/definition",
            Self::TypeDefinition => "textDocument/typeDefinition",
            Self::Implementation => "textDocument/implementation",
            Self::Declaration => "textDocument/declaration",
            Self::References => "textDocument/references",
            Self::Hover => "textDocument/hover",
            Self::Completions(_) | Self::ResolveCompletion(_) => "textDocument/completion",
            Self::InlineCompletion => "textDocument/inlineCompletion",
            Self::Signature => "textDocument/signatureHelp",
            Self::CodeActions | Self::SourceActions(_) => "textDocument/codeAction",
            Self::Rename(_) => "textDocument/rename",
            Self::Format => "textDocument/formatting",
            Self::FormatSelection => "textDocument/rangeFormatting",
            Self::FormatOnType(_) => "textDocument/onTypeFormatting",
            Self::PrepareRename => "textDocument/rename",
            Self::Folds => "textDocument/foldingRange",
            Self::SelectionRanges => "textDocument/selectionRange",
            Self::Symbols => "textDocument/documentSymbol",
            Self::Hints(_) => "textDocument/inlayHint",
            Self::Semantics => "textDocument/semanticTokens",
            Self::Occurrences => "textDocument/documentHighlight",
            Self::Lenses | Self::ResolveLens(_) => "textDocument/codeLens",
            Self::PrepareCalls(_) | Self::Calls(..) => "textDocument/prepareCallHierarchy",
            Self::WorkspaceSymbols(_) => "workspace/symbol",
            Self::WillSave => "textDocument/willSaveWaitUntil",
            Self::WillRenameFiles(_) => "workspace/willRenameFiles",
        }
    }

    /// The method this is asked by, for the log.
    pub(super) fn method(&self) -> &'static str {
        match self {
            Self::Semantics => "textDocument/semanticTokens/full",
            Self::ResolveCompletion(_) => "completionItem/resolve",
            Self::PrepareRename => "textDocument/prepareRename",
            Self::ResolveLens(_) => "codeLens/resolve",
            Self::Calls(Calls::Incoming, _) => "callHierarchy/incomingCalls",
            Self::Calls(Calls::Outgoing, _) => "callHierarchy/outgoingCalls",
            request => request.capability(),
        }
    }

    /// Whether a server that said `capabilities` answers this about `document`.
    ///
    /// A server asked what it never offered answers with an error at best,
    /// and a save that waits on the answer waits on a refusal.
    pub(super) fn is_offered(
        &self,
        capabilities: &Capabilities,
        document: Option<Document>,
    ) -> bool {
        if let Self::WillRenameFiles(moves) = self {
            return moves
                .iter()
                .any(|(from, _)| capabilities.file_operation(self.capability(), from));
        }
        if !capabilities.offers(self.capability(), document) {
            return false;
        }
        match self {
            Self::ResolveCompletion(_) => capabilities.resolves_completions(document),
            Self::PrepareRename => capabilities.prepares_renames(document),
            Self::ResolveLens(_) => capabilities.resolves_lenses(document),
            Self::SourceActions(kind) => capabilities.offers_action_kind(kind, document),
            _ => true,
        }
    }

    /// This request with every place named in it counted the server's way.
    ///
    /// Only a request carrying a span of its own has anything to translate;
    /// the place the rest are asked about is translated by the client as it
    /// asks them.
    pub(super) fn encoded(&self, path: &Path, files: &mut Files) -> Self {
        match self {
            Self::Hints(span) => {
                Self::Hints(files.encode(path, span.start)..files.encode(path, span.end))
            }
            request => request.clone(),
        }
    }

    /// The message asking this under `id`, about what `asking` says.
    ///
    /// A question carrying something a server handed out of another kind
    /// than it asks about has nothing to ask with.
    pub(super) fn message(&self, id: i64, asking: Asking) -> Option<Value> {
        let document = TextDocumentIdentifier::new(uri::typed(asking.path));
        let place = TextDocumentPositionParams::new(document.clone(), wire(asking.at));
        let goto = GotoDefinitionParams {
            text_document_position_params: place.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        Some(match self {
            Self::Definition => rpc::request::<GotoDefinition>(id, goto),
            Self::TypeDefinition => rpc::request::<GotoTypeDefinition>(id, goto),
            Self::Implementation => rpc::request::<GotoImplementation>(id, goto),
            Self::Declaration => rpc::request::<GotoDeclaration>(id, goto),
            Self::References => rpc::request::<References>(
                id,
                ReferenceParams {
                    text_document_position: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                    context: ReferenceContext {
                        include_declaration: false,
                    },
                },
            ),
            Self::Hover => rpc::request::<HoverRequest>(
                id,
                HoverParams {
                    text_document_position_params: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ),
            Self::Completions(trigger) => rpc::request::<CompletionRequest>(
                id,
                CompletionParams {
                    text_document_position: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                    context: Some(match trigger {
                        Trigger::Invoked => CompletionContext {
                            trigger_kind: CompletionTriggerKind::INVOKED,
                            trigger_character: None,
                        },
                        Trigger::Character(typed) => CompletionContext {
                            trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                            trigger_character: Some(typed.to_string()),
                        },
                        Trigger::Incomplete => CompletionContext {
                            trigger_kind: CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS,
                            trigger_character: None,
                        },
                    }),
                },
            ),
            Self::InlineCompletion => rpc::request::<InlineCompletionRequest>(
                id,
                InlineCompletionParams {
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    text_document_position: place,
                    context: InlineCompletionContext {
                        trigger_kind: InlineCompletionTriggerKind::Automatic,
                        selected_completion_info: None,
                    },
                },
            ),
            Self::ResolveCompletion(handle) => match &handle.0 {
                Handed::Completion(item) => {
                    rpc::request::<ResolveCompletionItem>(id, *item.clone())
                }
                _ => return None,
            },
            Self::Signature => rpc::request::<SignatureHelpRequest>(
                id,
                SignatureHelpParams {
                    context: None,
                    text_document_position_params: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ),
            Self::CodeActions => rpc::request::<CodeActionRequest>(
                id,
                CodeActionParams {
                    text_document: document,
                    range: wire_range(asking.selection),
                    context: CodeActionContext {
                        diagnostics: asking.diagnostics,
                        only: None,
                        trigger_kind: None,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::SourceActions(kind) => rpc::request::<CodeActionRequest>(
                id,
                CodeActionParams {
                    text_document: document,
                    range: wire_range(asking.selection),
                    context: CodeActionContext {
                        diagnostics: Vec::new(),
                        only: Some(vec![lsp_types::CodeActionKind::from(kind.clone())]),
                        trigger_kind: None,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::Rename(name) => rpc::request::<Rename>(
                id,
                RenameParams {
                    text_document_position: place,
                    new_name: name.clone(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ),
            Self::Format => rpc::request::<Formatting>(
                id,
                DocumentFormattingParams {
                    text_document: document,
                    options: formatting(asking.indent),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ),
            Self::FormatSelection => rpc::request::<RangeFormatting>(
                id,
                DocumentRangeFormattingParams {
                    text_document: document,
                    range: wire_range(asking.selection),
                    options: formatting(asking.indent),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ),
            Self::FormatOnType(typed) => rpc::request::<OnTypeFormatting>(
                id,
                DocumentOnTypeFormattingParams {
                    text_document_position: place,
                    ch: typed.to_string(),
                    options: formatting(asking.indent),
                },
            ),
            Self::PrepareRename => rpc::request::<PrepareRenameRequest>(id, place),
            Self::Folds => rpc::request::<FoldingRangeRequest>(
                id,
                FoldingRangeParams {
                    text_document: document,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::SelectionRanges => rpc::request::<SelectionRangeRequest>(
                id,
                SelectionRangeParams {
                    text_document: document,
                    positions: vec![wire(asking.at)],
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::Symbols => rpc::request::<DocumentSymbolRequest>(
                id,
                DocumentSymbolParams {
                    text_document: document,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::Hints(span) => rpc::request::<InlayHintRequest>(
                id,
                InlayHintParams {
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    text_document: document,
                    range: wire_range(span.clone()),
                },
            ),
            Self::Semantics => rpc::request::<SemanticTokensFullRequest>(
                id,
                SemanticTokensParams {
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                    text_document: document,
                },
            ),
            Self::Occurrences => rpc::request::<DocumentHighlightRequest>(
                id,
                DocumentHighlightParams {
                    text_document_position_params: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::Lenses => rpc::request::<CodeLensRequest>(
                id,
                CodeLensParams {
                    text_document: document,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
            ),
            Self::ResolveLens(handle) => match &handle.0 {
                Handed::Lens(lens) => rpc::request::<CodeLensResolve>(id, *lens.clone()),
                _ => return None,
            },
            Self::PrepareCalls(_) => rpc::request::<CallHierarchyPrepare>(
                id,
                CallHierarchyPrepareParams {
                    text_document_position_params: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                },
            ),
            Self::Calls(direction, handle) => {
                let Handed::Call(item) = &handle.0 else {
                    return None;
                };
                let item = *item.clone();
                match direction {
                    Calls::Incoming => rpc::request::<CallHierarchyIncomingCalls>(
                        id,
                        CallHierarchyIncomingCallsParams {
                            item,
                            work_done_progress_params: WorkDoneProgressParams::default(),
                            partial_result_params: PartialResultParams::default(),
                        },
                    ),
                    Calls::Outgoing => rpc::request::<CallHierarchyOutgoingCalls>(
                        id,
                        CallHierarchyOutgoingCallsParams {
                            item,
                            work_done_progress_params: WorkDoneProgressParams::default(),
                            partial_result_params: PartialResultParams::default(),
                        },
                    ),
                }
            }
            Self::WorkspaceSymbols(query) => rpc::request::<WorkspaceSymbolRequest>(
                id,
                WorkspaceSymbolParams {
                    partial_result_params: PartialResultParams::default(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    query: query.clone(),
                },
            ),
            Self::WillSave => rpc::request::<WillSaveWaitUntil>(
                id,
                WillSaveTextDocumentParams {
                    text_document: document,
                    reason: TextDocumentSaveReason::MANUAL,
                },
            ),
            Self::WillRenameFiles(moves) => rpc::request::<WillRenameFiles>(
                id,
                RenameFilesParams {
                    files: moves
                        .iter()
                        .map(|(from, to)| FileRename {
                            old_uri: uri::of(from),
                            new_uri: uri::of(to),
                        })
                        .collect(),
                },
            ),
        })
    }

    /// What a server's reply to this request, made about `path`, comes to.
    ///
    /// `legend` is what the server said its token types are, in the order it
    /// numbers them; only a reply about semantics is read through it. A reply
    /// that is not the shape its method's result is comes to nothing.
    pub(super) fn read(&self, path: &Path, result: Value, legend: &Legend) -> Option<Answer> {
        Some(match self {
            Self::Definition => Answer::Locations(gone_to(rpc::result::<GotoDefinition>(result)?)),
            Self::TypeDefinition => {
                Answer::Locations(gone_to(rpc::result::<GotoTypeDefinition>(result)?))
            }
            Self::Implementation => {
                Answer::Locations(gone_to(rpc::result::<GotoImplementation>(result)?))
            }
            Self::Declaration => {
                Answer::Locations(gone_to(rpc::result::<GotoDeclaration>(result)?))
            }
            Self::References => Answer::Locations(
                rpc::result::<References>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(location)
                    .collect(),
            ),
            Self::Hover => Answer::Hover(
                rpc::result::<HoverRequest>(result)?
                    .map(|hover| hover_text(hover.contents))
                    .unwrap_or_default(),
            ),
            Self::Completions(_) => {
                let found = rpc::result::<CompletionRequest>(result)?;
                Answer::Completions {
                    incomplete: matches!(&found, Some(CompletionResponse::List(list)) if list.is_incomplete),
                    items: completions(found),
                }
            }
            Self::InlineCompletion => {
                let found = rpc::result::<InlineCompletionRequest>(result)?;
                let items = match found {
                    Some(InlineCompletionResponse::Array(items)) => items,
                    Some(InlineCompletionResponse::List(list)) => list.items,
                    None => Vec::new(),
                };
                Answer::Inline(
                    items
                        .into_iter()
                        .map(|item| Prediction {
                            range: item.range.map(range).unwrap_or_default(),
                            text: item.insert_text,
                            version: 0,
                        })
                        .collect(),
                )
            }
            Self::ResolveCompletion(_) => {
                let item = rpc::result::<ResolveCompletionItem>(result)?;
                Answer::Resolved(Box::new(completion(item)?))
            }
            Self::Signature => Answer::Signature(
                rpc::result::<SignatureHelpRequest>(result)?
                    .and_then(signature)
                    .unwrap_or_default(),
            ),
            Self::FormatSelection => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: text_edits(rpc::result::<RangeFormatting>(result)?.unwrap_or_default()),
            }]),
            Self::FormatOnType(_) => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: text_edits(rpc::result::<OnTypeFormatting>(result)?.unwrap_or_default()),
            }]),
            Self::PrepareRename => match rpc::result::<PrepareRenameRequest>(result)? {
                None => Answer::Refused,
                Some(PrepareRenameResponse::Range(span)) => Answer::Renamable {
                    span: Some(range(span)),
                    placeholder: None,
                },
                Some(PrepareRenameResponse::RangeWithPlaceholder {
                    range: span,
                    placeholder,
                }) => Answer::Renamable {
                    span: Some(range(span)),
                    placeholder: Some(placeholder),
                },
                Some(PrepareRenameResponse::DefaultBehavior { .. }) => Answer::Renamable {
                    span: None,
                    placeholder: None,
                },
            },
            Self::Folds => Answer::Folds(
                rpc::result::<FoldingRangeRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|fold| fold.end_line > fold.start_line)
                    .map(|fold| fold.start_line as usize + 1..fold.end_line as usize + 1)
                    .collect(),
            ),
            Self::SelectionRanges => Answer::Selections(
                rpc::result::<SelectionRangeRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|innermost| {
                        std::iter::successors(Some(innermost), |span| {
                            span.parent.as_deref().cloned()
                        })
                        .map(|span| range(span.range))
                        .collect()
                    })
                    .unwrap_or_default(),
            ),
            Self::CodeActions => Answer::CodeActions(
                rpc::result::<CodeActionRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(code_action)
                    .collect(),
            ),
            Self::SourceActions(_) => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: rpc::result::<CodeActionRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(code_action)
                    .flat_map(|action| action.edits)
                    .find_map(|change| match change {
                        WorkspaceChange::Edit(file) if file.path == path => Some(file.edits),
                        _ => None,
                    })
                    .unwrap_or_default(),
            }]),
            Self::Rename(_) => Answer::Changes(
                rpc::result::<Rename>(result)?
                    .map(|edit| workspace_edit(&edit))
                    .unwrap_or_default(),
            ),
            Self::Format => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: text_edits(rpc::result::<Formatting>(result)?.unwrap_or_default()),
            }]),
            Self::Symbols => {
                Answer::Symbols(symbols(rpc::result::<DocumentSymbolRequest>(result)?))
            }
            Self::Hints(_) => Answer::Hints(
                rpc::result::<InlayHintRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(hint)
                    .collect(),
            ),
            Self::Semantics => Answer::Semantics(semantics(
                &match rpc::result::<SemanticTokensFullRequest>(result)? {
                    None => Vec::new(),
                    Some(SemanticTokensResult::Tokens(tokens)) => tokens.data,
                    Some(SemanticTokensResult::Partial(partial)) => partial.data,
                },
                legend,
            )),
            Self::Occurrences => Answer::Occurrences(
                rpc::result::<DocumentHighlightRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .map(|highlight| range(highlight.range))
                    .collect(),
            ),
            Self::Lenses => Answer::Lenses(
                rpc::result::<CodeLensRequest>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .map(lens)
                    .collect(),
            ),
            Self::ResolveLens(_) => {
                Answer::Lenses(vec![lens(rpc::result::<CodeLensResolve>(result)?)])
            }
            Self::PrepareCalls(_) => Answer::CallItems(
                rpc::result::<CallHierarchyPrepare>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| Handle(Handed::Call(Box::new(item))))
                    .collect(),
            ),
            Self::Calls(Calls::Incoming, _) => Answer::Named(
                rpc::result::<CallHierarchyIncomingCalls>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|call| {
                        let span = call
                            .from_ranges
                            .first()
                            .copied()
                            .unwrap_or(call.from.selection_range);
                        call_item(&call.from, span)
                    })
                    .collect(),
            ),
            Self::Calls(Calls::Outgoing, _) => Answer::Named(
                rpc::result::<CallHierarchyOutgoingCalls>(result)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|call| call_item(&call.to, call.to.selection_range))
                    .collect(),
            ),
            Self::WorkspaceSymbols(_) => Answer::Named(workspace_symbols(rpc::result::<
                WorkspaceSymbolRequest,
            >(result)?)),
            Self::WillSave => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: text_edits(rpc::result::<WillSaveWaitUntil>(result)?.unwrap_or_default()),
            }]),
            Self::WillRenameFiles(_) => Answer::Changes(
                rpc::result::<WillRenameFiles>(result)?
                    .map(|edit| workspace_edit(&edit))
                    .unwrap_or_default(),
            ),
        })
    }
}

/// What a server said in reply.
#[derive(Clone, Debug)]
pub enum Answer {
    /// Places in files: a definition, a declaration, a list of uses.
    Locations(Vec<Location>),
    /// What the server says about a place, as text.
    Hover(String),
    /// What could be written where the cursor is, and whether the server
    /// said there is more than it sent, to be asked for as the reader types.
    Completions {
        /// What could be written.
        items: Vec<Completion>,
        /// Whether the list is not all there is.
        incomplete: bool,
    },
    /// Text predicted for insertion at the cursor.
    Inline(Vec<Prediction>),
    /// One of those, filled in with what the server left out of the list.
    Resolved(Box<Completion>),
    /// The signature of the call the cursor is inside.
    Signature(Signature),
    /// Whether the symbol asked about can be renamed: the span of its name,
    /// and what to offer as the new one, when the server said.
    Renamable {
        /// Where the name is, when the server said.
        span: Option<Range<Position>>,
        /// What the name is, when the server said.
        placeholder: Option<String>,
    },
    /// The runs of lines that fold together, as the lines each would hide.
    Folds(Vec<Range<usize>>),
    /// The spans the selection can grow through, from the smallest out.
    Selections(Vec<Range<Position>>),
    /// The fixes and refactors offered where the cursor is.
    CodeActions(Vec<CodeAction>),
    /// Changes to make to one file, from a formatter.
    Edits(Vec<FileEdit>),
    /// Changes to make across the workspace, in order, from a rename.
    Changes(Vec<WorkspaceChange>),
    /// The symbols a file declares.
    Symbols(Vec<Symbol>),
    /// What the server would write into the lines it was asked about.
    Hints(Vec<crate::hint::Hint>),
    /// What the server makes of every name in the file, as spans to colour.
    Semantics(Vec<Semantic>),
    /// Every place in the file the symbol asked about is read or written.
    Occurrences(Vec<Range<Position>>),
    /// The notes above the file's declarations, or one of them resolved.
    Lenses(Vec<Lens>),
    /// The symbols a place names, as the server will follow their calls.
    CallItems(Vec<Handle>),
    /// Named places in any file: callers, callees, symbols of the workspace.
    Named(Vec<NamedLocation>),
    /// The server answered with an error, and so with nothing to act on.
    Refused,
}

impl Answer {
    /// Counts every place the answer names the editor's way.
    ///
    /// The file a place is in is the file it is counted against, which is
    /// not always the file that was asked about: a definition is somewhere
    /// else by definition, and a rename is in as many files as it touches.
    pub(super) fn decode(&mut self, path: &Path, files: &mut Files) {
        match self {
            Self::Locations(found) => {
                for location in found {
                    let at = location.path.clone();
                    location.range = files.decode_span(&at, location.range.clone());
                    location.origin = location
                        .origin
                        .clone()
                        .map(|span| files.decode_span(path, span));
                }
            }
            Self::Hover(_) | Self::Signature(_) | Self::Folds(_) => {}
            Self::Renamable { span, .. } => {
                *span = span.clone().map(|span| files.decode_span(path, span));
            }
            Self::Selections(spans) => {
                for span in spans {
                    *span = files.decode_span(path, span.clone());
                }
            }
            Self::Completions { items, .. } => {
                for item in items {
                    decode_completion(item, path, files);
                }
            }
            Self::Inline(items) => {
                for item in items {
                    item.range = files.decode_span(path, item.range.clone());
                }
            }
            Self::Resolved(item) => decode_completion(item, path, files),
            Self::CodeActions(actions) => {
                for action in actions {
                    decode_changes(&mut action.edits, files);
                }
            }
            Self::Edits(edited) => decode_edits(edited, files),
            Self::Changes(changes) => decode_changes(changes, files),
            Self::Symbols(symbols) => {
                for symbol in symbols {
                    symbol.position = files.decode(path, symbol.position);
                }
            }
            Self::Hints(hints) => {
                for hint in hints {
                    hint.position = files.decode(path, hint.position);
                }
            }
            Self::Semantics(spans) => {
                for (span, ..) in spans {
                    *span = files.decode_span(path, span.clone());
                }
            }
            Self::Occurrences(spans) => {
                for span in spans {
                    *span = files.decode_span(path, span.clone());
                }
            }
            Self::Lenses(lenses) => {
                for lens in lenses {
                    lens.position = files.decode(path, lens.position);
                }
            }
            Self::Named(found) => {
                for named in found {
                    let at = named.location.path.clone();
                    named.location.range = files.decode_span(&at, named.location.range.clone());
                }
            }
            Self::CallItems(_) | Self::Refused => {}
        }
    }

    /// Whether the server answered with nothing.
    ///
    /// A server that was asked something it has no opinion about answers
    /// the question and says nothing in it, which is not the same as an
    /// answer worth showing.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Locations(found) => found.is_empty(),
            Self::Hover(text) => text.is_empty(),
            Self::Signature(signature) => signature.label.is_empty(),
            Self::Renamable { .. } => false,
            Self::Folds(folds) => folds.is_empty(),
            Self::Selections(spans) => spans.is_empty(),
            Self::Completions { items, .. } => items.is_empty(),
            Self::Inline(items) => items.is_empty(),
            Self::Resolved(_) => false,
            Self::CodeActions(actions) => actions.is_empty(),
            Self::Edits(files) => files.iter().all(|file| file.edits.is_empty()),
            Self::Changes(changes) => changes.iter().all(WorkspaceChange::is_empty),
            Self::Symbols(symbols) => symbols.is_empty(),
            Self::Hints(hints) => hints.is_empty(),
            Self::Semantics(spans) => spans.is_empty(),
            Self::Occurrences(spans) => spans.is_empty(),
            Self::Lenses(lenses) => lenses.is_empty(),
            Self::CallItems(items) => items.is_empty(),
            Self::Named(found) => found.is_empty(),
            Self::Refused => true,
        }
    }
}

/// Counts the places one completion names the editor's way.
fn decode_completion(item: &mut Completion, path: &Path, files: &mut Files) {
    item.range = item.range.clone().map(|span| files.decode_span(path, span));
    item.replace = item
        .replace
        .clone()
        .map(|span| files.decode_span(path, span));
    for (span, _) in &mut item.extra {
        *span = files.decode_span(path, span.clone());
    }
}

/// Counts the places every edit among `changes` names the editor's way.
pub(super) fn decode_changes(changes: &mut [WorkspaceChange], files: &mut Files) {
    for change in changes {
        if let WorkspaceChange::Edit(file) = change {
            decode_edits(std::slice::from_mut(file), files);
        }
    }
}

/// Counts the places every one of `edited` names the editor's way.
fn decode_edits(edited: &mut [FileEdit], files: &mut Files) {
    for file in edited {
        let path = file.path.clone();
        for (span, _) in &mut file.edits {
            *span = files.decode_span(&path, span.clone());
        }
    }
}

/// The signature of the call the cursor is inside, and where in it the
/// cursor's argument is.
#[derive(Clone, Debug, Default)]
pub struct Signature {
    /// The signature, as the server writes it.
    pub label: String,
    /// The span of the label that names the parameter being written, in
    /// characters, when the server said which it is.
    pub active: Option<Range<usize>>,
    /// What the server says about that parameter.
    pub parameter: String,
    /// What the server says about the function as a whole.
    pub documentation: String,
}

/// One place in one file.
#[derive(Clone, Debug)]
pub struct Location {
    /// Where the file lives.
    pub path: PathBuf,
    /// The span in it.
    pub range: Range<Position>,
    /// The span in the asking file that leads here, when the server said.
    ///
    /// This is what a link is drawn under: the server knows where the name
    /// it resolved begins and ends, and it knows it better than a walk over
    /// the characters around the pointer does.
    pub origin: Option<Range<Position>>,
}

/// One thing that could be written where the cursor is.
#[derive(Clone, Debug)]
pub struct Completion {
    /// What the list calls it.
    pub label: String,
    /// What the list narrows it by as the reader types, which is the label
    /// unless the server said otherwise.
    pub filter: String,
    /// What is said beside it: a type, a signature, a module.
    pub detail: String,
    /// What the server says about it at length, once it has said.
    pub documentation: String,
    /// What goes into the buffer when it is chosen.
    pub insert: String,
    /// The places in that to fill in afterwards, in order, each as the
    /// spans of it the place covers, in characters; empty for plain text.
    pub stops: Vec<Vec<Range<usize>>>,
    /// What kind of thing it is.
    pub kind: CompletionKind,
    /// What follows the label without a gap: a signature or a type
    /// annotation, when the server sends it apart from the detail.
    pub signature: String,
    /// What the server ranks it by among the others, which is the label
    /// unless the server said otherwise.
    pub sort: String,
    /// The span it replaces when it is inserted before the text that follows
    /// the cursor, when the server named one.
    pub range: Option<Range<Position>>,
    /// The span it replaces when it is taken over the rest of the word as
    /// well, when the server named one apart from the first.
    pub replace: Option<Range<Position>>,
    /// Whether the server says the thing is out of date.
    pub deprecated: bool,
    /// Whether the server says it is the one most likely wanted.
    pub preselect: bool,
    /// The characters that take it when typed while it is selected.
    pub commit: Vec<char>,
    /// Changes elsewhere in the file that choosing it brings along: the
    /// import a name needs, most often.
    pub extra: Vec<(Range<Position>, String)>,
    /// The server's own record of it, to have it filled in by.
    pub handle: Handle,
}

/// What kind of thing a completion offers to write.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompletionKind {
    /// A method of a type.
    Method,
    /// A free function.
    Function,
    /// A constructor of a type.
    Constructor,
    /// A field of a record.
    Field,
    /// A local or a static variable.
    Variable,
    /// A class.
    Class,
    /// An interface or a trait.
    Interface,
    /// A module or a namespace.
    Module,
    /// A property of an object.
    Property,
    /// A unit of measure.
    Unit,
    /// A literal value.
    Value,
    /// An enumeration.
    Enum,
    /// Plain text, such as a word of the file.
    Text,
    /// A reserved word.
    Keyword,
    /// A snippet of text with places to fill in.
    Snippet,
    /// A colour.
    Color,
    /// A file.
    File,
    /// A reference to something elsewhere.
    Reference,
    /// A directory.
    Folder,
    /// One variant of an enumeration.
    EnumMember,
    /// A constant.
    Constant,
    /// A struct.
    Struct,
    /// An event.
    Event,
    /// An operator.
    Operator,
    /// A parameter of a generic type.
    TypeParameter,
    /// Something the server did not say.
    Other,
}

impl CompletionKind {
    /// Whether what it names is called, and so is followed by parentheses.
    pub fn callable(self) -> bool {
        matches!(self, Self::Function | Self::Method | Self::Constructor)
    }
}

impl From<CompletionItemKind> for CompletionKind {
    /// The kind a server named, in the editor's own terms.
    fn from(kind: CompletionItemKind) -> Self {
        match kind {
            CompletionItemKind::METHOD => Self::Method,
            CompletionItemKind::FUNCTION => Self::Function,
            CompletionItemKind::CONSTRUCTOR => Self::Constructor,
            CompletionItemKind::FIELD => Self::Field,
            CompletionItemKind::VARIABLE => Self::Variable,
            CompletionItemKind::CLASS => Self::Class,
            CompletionItemKind::INTERFACE => Self::Interface,
            CompletionItemKind::MODULE => Self::Module,
            CompletionItemKind::PROPERTY => Self::Property,
            CompletionItemKind::UNIT => Self::Unit,
            CompletionItemKind::VALUE => Self::Value,
            CompletionItemKind::ENUM => Self::Enum,
            CompletionItemKind::TEXT => Self::Text,
            CompletionItemKind::KEYWORD => Self::Keyword,
            CompletionItemKind::SNIPPET => Self::Snippet,
            CompletionItemKind::COLOR => Self::Color,
            CompletionItemKind::FILE => Self::File,
            CompletionItemKind::REFERENCE => Self::Reference,
            CompletionItemKind::FOLDER => Self::Folder,
            CompletionItemKind::ENUM_MEMBER => Self::EnumMember,
            CompletionItemKind::CONSTANT => Self::Constant,
            CompletionItemKind::STRUCT => Self::Struct,
            CompletionItemKind::EVENT => Self::Event,
            CompletionItemKind::OPERATOR => Self::Operator,
            CompletionItemKind::TYPE_PARAMETER => Self::TypeParameter,
            _ => Self::Other,
        }
    }
}

impl Completion {
    /// A word of the file itself, offered where no server has anything.
    pub fn word(word: &str) -> Self {
        Self {
            label: word.to_owned(),
            filter: word.to_owned(),
            detail: String::new(),
            documentation: String::new(),
            insert: word.to_owned(),
            stops: Vec::new(),
            kind: CompletionKind::Text,
            signature: String::new(),
            sort: word.to_owned(),
            range: None,
            replace: None,
            deprecated: false,
            preselect: false,
            commit: Vec::new(),
            extra: Vec::new(),
            handle: Handle(Handed::Word),
        }
    }
}

/// One fix or refactor the server offers.
#[derive(Clone, Debug)]
pub struct CodeAction {
    /// What the menu calls it.
    pub title: String,
    /// The changes it makes, in order, when it carries them itself.
    pub edits: Vec<WorkspaceChange>,
    /// The server command to run after applying the edit, when present.
    pub command: Option<Command>,
}

/// A command a server offers as all or part of a code action.
#[derive(Clone, Debug)]
pub struct Command {
    /// The identifier the server recognizes.
    pub name: String,
    /// The arguments returned by the server, which only it reads.
    pub arguments: Vec<Value>,
}

/// The changes one file is asked to take.
#[derive(Clone, Debug)]
pub struct FileEdit {
    /// Where the file lives.
    pub path: PathBuf,
    /// The spans to replace, and what to put in their place.
    pub edits: Vec<(Range<Position>, String)>,
}

/// One change a server asks for across the workspace: text to change in a
/// file, or a file to make, move or take away.
#[derive(Clone, Debug)]
pub enum WorkspaceChange {
    /// Text to change in one file.
    Edit(FileEdit),
    /// A file to make.
    Create {
        /// Where it goes.
        path: PathBuf,
        /// Whether a file already there is emptied rather than kept.
        overwrite: bool,
        /// Whether a file already there is left alone rather than refused.
        ignore_if_exists: bool,
    },
    /// A file or directory to move.
    Rename {
        /// Where it is.
        from: PathBuf,
        /// Where it goes.
        to: PathBuf,
        /// Whether whatever is already at `to` is replaced rather than refused.
        overwrite: bool,
        /// Whether whatever is already at `to` means the move is skipped.
        ignore_if_exists: bool,
    },
    /// A file or directory to take away.
    Delete {
        /// Where it is.
        path: PathBuf,
        /// Whether nothing being there is fine rather than a failure.
        ignore_if_not_exists: bool,
    },
}

impl WorkspaceChange {
    /// Whether this changes nothing: an edit with no edits in it.
    fn is_empty(&self) -> bool {
        matches!(self, Self::Edit(file) if file.edits.is_empty())
    }
}

/// One symbol a file declares.
#[derive(Clone, Debug)]
pub struct Symbol {
    /// What it is called.
    pub name: String,
    /// What is said beside it: a signature, or the symbol that holds it.
    pub detail: String,
    /// What kind of thing it is, as one word.
    pub kind: &'static str,
    /// Where it is declared.
    pub position: Position,
    /// The full declaration, including its body.
    pub range: Range<Position>,
    /// How many symbols it sits inside.
    pub depth: usize,
}

/// A place in some file, and what is found there.
#[derive(Clone, Debug)]
pub struct NamedLocation {
    /// What is found there.
    pub name: String,
    /// What is said beside it: a signature, or the symbol that holds it.
    pub detail: String,
    /// What kind of thing it is, as one word.
    pub kind: &'static str,
    /// Where it is.
    pub location: Location,
}

/// A note a server puts above a declaration: a count of uses, a way to run it.
#[derive(Clone, Debug)]
pub struct Lens {
    /// Where the declaration it is about begins.
    pub position: Position,
    /// What it says, once the server has said it.
    pub title: Option<String>,
    /// The server's own record of it, to resolve it by.
    pub handle: Handle,
}

/// One end of a span, in the editor's own terms.
pub(super) fn position(at: lsp_types::Position) -> Position {
    Position::new(at.line as usize, at.character as usize)
}

/// One span, in the editor's own terms.
pub(super) fn range(span: lsp_types::Range) -> Range<Position> {
    position(span.start)..position(span.end)
}

/// One end of a span, in the protocol's terms.
pub(super) fn wire(at: Position) -> lsp_types::Position {
    lsp_types::Position::new(at.line as u32, at.column as u32)
}

/// One span, in the protocol's terms.
fn wire_range(span: Range<Position>) -> lsp_types::Range {
    lsp_types::Range::new(wire(span.start), wire(span.end))
}

/// How a file is to be laid out, as a formatter is told it.
fn formatting(indent: Indent) -> FormattingOptions {
    FormattingOptions {
        tab_size: indent.width as u32,
        insert_spaces: !indent.tabs,
        ..FormattingOptions::default()
    }
}

/// The text of a server's documentation, whichever way it wrote it.
fn documentation(written: Option<Documentation>) -> String {
    match written {
        Some(Documentation::String(text)) => text,
        Some(Documentation::MarkupContent(markup)) => markup.value,
        None => String::new(),
    }
    .trim()
    .to_owned()
}

/// The active signature of `help`, with the parameter being written in it.
///
/// A parameter the server names by offsets counts them in UTF-16 code
/// units, which are turned into characters of the label here.
fn signature(help: SignatureHelp) -> Option<Signature> {
    let active = help.active_signature.unwrap_or_default() as usize;
    let mut signatures = help.signatures;
    let chosen = match active < signatures.len() {
        true => signatures.swap_remove(active),
        false => signatures.into_iter().next()?,
    };
    let index = chosen
        .active_parameter
        .or(help.active_parameter)
        .unwrap_or_default() as usize;
    let parameter = chosen.parameters.unwrap_or_default().into_iter().nth(index);
    let active = parameter
        .as_ref()
        .and_then(|parameter| match &parameter.label {
            ParameterLabel::Simple(name) => chosen.label.find(name.as_str()).map(|byte| {
                let start = chosen.label[..byte].chars().count();
                start..start + name.chars().count()
            }),
            ParameterLabel::LabelOffsets([start, end]) => {
                let at = |units: u32| {
                    let mut counted = 0;
                    chosen
                        .label
                        .chars()
                        .take_while(|ch| {
                            counted += ch.len_utf16() as u32;
                            counted <= units
                        })
                        .count()
                };
                Some(at(*start)..at(*end))
            }
        });
    Some(Signature {
        active,
        parameter: documentation(parameter.and_then(|parameter| parameter.documentation)),
        documentation: documentation(chosen.documentation),
        label: chosen.label,
    })
}

/// One place a server named, if it is a file the editor can open.
fn location(found: lsp_types::Location) -> Option<Location> {
    Some(Location {
        path: uri::path_of(&found.uri)?,
        range: range(found.range),
        origin: None,
    })
}

/// The places a go-to answer names: a location, a list of them, or links.
fn gone_to(found: Option<GotoDefinitionResponse>) -> Vec<Location> {
    match found {
        None => Vec::new(),
        Some(GotoDefinitionResponse::Scalar(found)) => location(found).into_iter().collect(),
        Some(GotoDefinitionResponse::Array(found)) => {
            found.into_iter().filter_map(location).collect()
        }
        Some(GotoDefinitionResponse::Link(links)) => links
            .into_iter()
            .filter_map(|link| {
                Some(Location {
                    path: uri::path_of(&link.target_uri)?,
                    range: range(link.target_selection_range),
                    origin: link.origin_selection_range.map(range),
                })
            })
            .collect(),
    }
}

/// What a hover says, with the protocol's wrappers taken off.
fn hover_text(contents: HoverContents) -> String {
    let marked = |marked: MarkedString| match marked {
        MarkedString::String(text) => text,
        MarkedString::LanguageString(code) => code.value,
    };
    let text = match contents {
        HoverContents::Scalar(part) => marked(part),
        HoverContents::Array(parts) => parts.into_iter().map(marked).collect::<Vec<_>>().join("\n"),
        HoverContents::Markup(markup) => markup.value,
    };
    text.trim().to_owned()
}

/// What could be written, in the order the server ranked it.
///
/// A server ranks by the sort text it gives each item, not by the order it
/// happens to list them in.
fn completions(found: Option<CompletionResponse>) -> Vec<Completion> {
    let mut items = match found {
        None => Vec::new(),
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
    };
    items.sort_by(|a, b| {
        let rank =
            |item: &CompletionItem| item.sort_text.clone().unwrap_or_else(|| item.label.clone());
        rank(a).cmp(&rank(b))
    });
    items.into_iter().filter_map(completion).collect()
}

/// One thing that could be written, in the editor's own terms.
fn completion(item: CompletionItem) -> Option<Completion> {
    let label = item.label.trim().to_owned();
    if label.is_empty() {
        return None;
    }
    let (span, replace, text) = match item.text_edit.clone() {
        Some(CompletionTextEdit::Edit(edit)) => {
            (Some(range(edit.range)), None, Some(edit.new_text))
        }
        Some(CompletionTextEdit::InsertAndReplace(edit)) => (
            Some(range(edit.insert)),
            Some(range(edit.replace)),
            Some(edit.new_text),
        ),
        None => (None, None, None),
    };
    let written = text
        .or_else(|| item.insert_text.clone())
        .unwrap_or_else(|| label.clone());
    let snippet = match item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
        true => crate::snippet::parse(&written),
        false => crate::snippet::Snippet {
            text: written,
            stops: Vec::new(),
        },
    };
    let labelled = item.label_details.clone().unwrap_or_default();
    Some(Completion {
        filter: item.filter_text.clone().unwrap_or_else(|| label.clone()),
        detail: labelled
            .description
            .clone()
            .or_else(|| item.detail.clone())
            .unwrap_or_default(),
        documentation: match item.documentation.clone() {
            Some(Documentation::String(text)) => text,
            Some(Documentation::MarkupContent(markup)) => markup.value,
            None => String::new(),
        }
        .trim()
        .to_owned(),
        kind: item
            .kind
            .map_or(CompletionKind::Other, CompletionKind::from),
        signature: labelled.detail.unwrap_or_default(),
        sort: item.sort_text.clone().unwrap_or_else(|| label.clone()),
        insert: snippet.text,
        stops: snippet.stops,
        range: span,
        replace,
        deprecated: item.deprecated == Some(true)
            || item
                .tags
                .as_ref()
                .is_some_and(|tags| tags.contains(&CompletionItemTag::DEPRECATED)),
        preselect: item.preselect == Some(true),
        commit: item
            .commit_characters
            .iter()
            .flatten()
            .filter_map(|typed| typed.chars().next())
            .collect(),
        extra: text_edits(item.additional_text_edits.clone().unwrap_or_default()),
        label,
        handle: Handle(Handed::Completion(Box::new(item))),
    })
}

/// One fix or refactor offered, if it can be taken.
///
/// An action the server says is disabled is one the menu has no use for,
/// and one that neither edits nor runs anything does nothing when taken.
fn code_action(offered: CodeActionOrCommand) -> Option<CodeAction> {
    let (title, edits, command) = match offered {
        CodeActionOrCommand::Command(command) => (command.title.clone(), Vec::new(), Some(command)),
        CodeActionOrCommand::CodeAction(action) => {
            if action.disabled.is_some() {
                return None;
            }
            let edits = action.edit.as_ref().map(workspace_edit).unwrap_or_default();
            (action.title, edits, action.command)
        }
    };
    let command = command.map(|command| Command {
        name: command.command,
        arguments: command.arguments.unwrap_or_default(),
    });
    (!edits.is_empty() || command.is_some()).then_some(CodeAction {
        title,
        edits,
        command,
    })
}

/// The changes a workspace edit asks for, in the order they are to be made.
///
/// Its document changes are an ordered list — a file is made before it is
/// written into, and moved after the edits that name it by its old name —
/// and are taken in that order, after the changes it lists by file.
pub(super) fn workspace_edit(edit: &WorkspaceEdit) -> Vec<WorkspaceChange> {
    let mut changes = Vec::new();
    for (uri, edits) in edit.changes.iter().flatten() {
        if let Some(path) = uri::path_of(uri) {
            changes.push(WorkspaceChange::Edit(FileEdit {
                path,
                edits: text_edits(edits.clone()),
            }));
        }
    }
    let operations = match &edit.document_changes {
        None => Vec::new(),
        Some(DocumentChanges::Edits(edits)) => edits
            .iter()
            .cloned()
            .map(DocumentChangeOperation::Edit)
            .collect(),
        Some(DocumentChanges::Operations(operations)) => operations.clone(),
    };
    changes.extend(operations.into_iter().filter_map(operation));
    changes
}

/// One document change, in the editor's own terms, if it names files the
/// editor can reach.
fn operation(operation: DocumentChangeOperation) -> Option<WorkspaceChange> {
    Some(match operation {
        DocumentChangeOperation::Edit(document) => WorkspaceChange::Edit(FileEdit {
            path: uri::path_of(&document.text_document.uri)?,
            edits: document
                .edits
                .into_iter()
                .map(|edit| match edit {
                    OneOf::Left(edit) => edit,
                    OneOf::Right(annotated) => annotated.text_edit,
                })
                .map(|edit| (range(edit.range), edit.new_text))
                .collect(),
        }),
        DocumentChangeOperation::Op(ResourceOp::Create(create)) => {
            let options = create.options.as_ref();
            WorkspaceChange::Create {
                path: uri::path_of(&create.uri)?,
                overwrite: options.and_then(|options| options.overwrite) == Some(true),
                ignore_if_exists: options.and_then(|options| options.ignore_if_exists)
                    == Some(true),
            }
        }
        DocumentChangeOperation::Op(ResourceOp::Rename(rename)) => {
            let options = rename.options.as_ref();
            WorkspaceChange::Rename {
                from: uri::path_of(&rename.old_uri)?,
                to: uri::path_of(&rename.new_uri)?,
                overwrite: options.and_then(|options| options.overwrite) == Some(true),
                ignore_if_exists: options.and_then(|options| options.ignore_if_exists)
                    == Some(true),
            }
        }
        DocumentChangeOperation::Op(ResourceOp::Delete(delete)) => WorkspaceChange::Delete {
            path: uri::path_of(&delete.uri)?,
            ignore_if_not_exists: delete
                .options
                .and_then(|options| options.ignore_if_not_exists)
                == Some(true),
        },
    })
}

/// Whether the editor can make every change `edit` asks for: whether every
/// file it names is one the editor can reach.
pub(super) fn is_supported(edit: &WorkspaceEdit) -> bool {
    let named = edit.changes.iter().flatten().count()
        + match &edit.document_changes {
            None => 0,
            Some(DocumentChanges::Edits(edits)) => edits.len(),
            Some(DocumentChanges::Operations(operations)) => operations.len(),
        };
    named > 0 && workspace_edit(edit).len() == named
}

/// The spans one file is asked to replace, and what with.
fn text_edits(edits: Vec<TextEdit>) -> Vec<(Range<Position>, String)> {
    edits
        .into_iter()
        .map(|edit| (range(edit.range), edit.new_text))
        .collect()
}

/// What a server would write into a line, in the editor's own terms.
///
/// A hint's label is either a string or a run of parts, each of which may
/// carry a link back into the source; what is drawn is the text of them,
/// because a hint is a note in the margin of a line and not a control.
fn hint(hint: InlayHint) -> Option<crate::hint::Hint> {
    let text = match hint.label {
        InlayHintLabel::String(text) => text,
        InlayHintLabel::LabelParts(parts) => parts.into_iter().map(|part| part.value).collect(),
    };
    let padded = format!(
        "{}{}{}",
        if hint.padding_left == Some(true) {
            " "
        } else {
            ""
        },
        text.trim(),
        if hint.padding_right == Some(true) {
            " "
        } else {
            ""
        },
    );
    (!padded.trim().is_empty()).then(|| crate::hint::Hint {
        position: position(hint.position),
        text: padded,
    })
}

/// A note above a declaration, said or waiting to be resolved.
fn lens(lens: CodeLens) -> Lens {
    Lens {
        position: position(lens.range.start),
        title: lens.command.as_ref().map(|command| command.title.clone()),
        handle: Handle(Handed::Lens(Box::new(lens))),
    }
}

/// The symbol a call hierarchy item names, shown at `span`.
fn call_item(item: &CallHierarchyItem, span: lsp_types::Range) -> Option<NamedLocation> {
    Some(NamedLocation {
        name: item.name.clone(),
        detail: item.detail.clone().unwrap_or_default(),
        kind: symbol_kind(item.kind),
        location: Location {
            path: uri::path_of(&item.uri)?,
            range: range(span),
            origin: None,
        },
    })
}

/// The symbols of a workspace a query matched, in the order the server ranked them.
///
/// A symbol whose location names only its file, which a server may send to
/// have it resolved later, is taken to be at the top of that file.
fn workspace_symbols(found: Option<WorkspaceSymbolResponse>) -> Vec<NamedLocation> {
    match found {
        None => Vec::new(),
        Some(WorkspaceSymbolResponse::Flat(symbols)) => {
            symbols.into_iter().filter_map(information).collect()
        }
        Some(WorkspaceSymbolResponse::Nested(symbols)) => symbols
            .into_iter()
            .filter_map(|symbol| {
                let (uri, span) = match symbol.location {
                    OneOf::Left(location) => (location.uri, location.range),
                    OneOf::Right(location) => (location.uri, lsp_types::Range::default()),
                };
                Some(NamedLocation {
                    name: symbol.name,
                    detail: symbol.container_name.unwrap_or_default(),
                    kind: symbol_kind(symbol.kind),
                    location: Location {
                        path: uri::path_of(&uri)?,
                        range: range(span),
                        origin: None,
                    },
                })
            })
            .collect(),
    }
}

/// One symbol a server listed flat, with where it is.
#[allow(deprecated)]
fn information(symbol: SymbolInformation) -> Option<NamedLocation> {
    Some(NamedLocation {
        location: location(symbol.location)?,
        name: symbol.name,
        detail: symbol.container_name.unwrap_or_default(),
        kind: symbol_kind(symbol.kind),
    })
}

/// The symbols a file declares, flattened in the order they appear.
#[allow(deprecated)]
fn symbols(found: Option<DocumentSymbolResponse>) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    match found {
        None => {}
        Some(DocumentSymbolResponse::Flat(flat)) => {
            symbols.extend(flat.into_iter().map(|symbol| Symbol {
                position: position(symbol.location.range.start),
                range: range(symbol.location.range),
                detail: symbol.container_name.unwrap_or_default(),
                kind: symbol_kind(symbol.kind),
                name: symbol.name,
                depth: 0,
            }));
        }
        Some(DocumentSymbolResponse::Nested(nested)) => collect_symbols(nested, 0, &mut symbols),
    }
    symbols
}

/// Walks one level of the symbol tree, and the levels under it.
fn collect_symbols(level: Vec<DocumentSymbol>, depth: usize, found: &mut Vec<Symbol>) {
    for symbol in level {
        found.push(Symbol {
            name: symbol.name,
            detail: symbol.detail.unwrap_or_default(),
            kind: symbol_kind(symbol.kind),
            position: position(symbol.selection_range.start),
            range: range(symbol.range),
            depth,
        });
        collect_symbols(symbol.children.unwrap_or_default(), depth + 1, found);
    }
}

/// What a symbol's kind is called.
fn symbol_kind(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::MODULE => "module",
        SymbolKind::CLASS => "class",
        SymbolKind::METHOD => "method",
        SymbolKind::PROPERTY => "property",
        SymbolKind::FIELD => "field",
        SymbolKind::CONSTRUCTOR => "constructor",
        SymbolKind::ENUM => "enum",
        SymbolKind::INTERFACE => "interface",
        SymbolKind::FUNCTION => "function",
        SymbolKind::VARIABLE => "variable",
        SymbolKind::CONSTANT => "constant",
        SymbolKind::STRUCT => "struct",
        SymbolKind::TYPE_PARAMETER => "type",
        _ => "",
    }
}

/// One span a server's semantic tokens come to: where it is, how it is
/// drawn, and whether it names something that can be assigned to again.
pub type Semantic = (Range<Position>, Highlight, bool);

/// What a server's semantic token legend means to the editor.
#[derive(Clone, Debug, Default)]
pub(super) struct Legend {
    /// The highlight each of the server's types means, in its own order.
    types: Vec<Option<Highlight>>,
    /// The bit of a token's modifiers that says it is mutable, if the server
    /// has such a modifier.
    mutable: u32,
}

/// The legend a server publishes, as what each of its types and modifiers means.
pub(super) fn legend(capabilities: &Capabilities) -> Legend {
    let Some(legend) = capabilities.legend() else {
        return Legend::default();
    };
    Legend {
        types: legend
            .token_types
            .iter()
            .map(|kind| Highlight::of_token(kind.as_str()))
            .collect(),
        mutable: legend
            .token_modifiers
            .iter()
            .position(|modifier| modifier.as_str() == "mutable")
            .map_or(0, |bit| 1 << bit),
    }
}

/// The spans a server's semantic tokens come to, in the file's own terms.
///
/// The protocol sends them relative to the token before each: a line down
/// from the last token's, a column along from it when they share a line, a
/// length, a type and its modifiers. A token whose type the editor draws no
/// differently is left out here rather than carried to the painter to be
/// discarded there.
pub(super) fn semantics(tokens: &[SemanticToken], legend: &Legend) -> Vec<Semantic> {
    let mut spans = Vec::new();
    let (mut line, mut column) = (0_usize, 0_usize);
    for token in tokens {
        let down = token.delta_line as usize;
        line += down;
        column = if down == 0 {
            column + token.delta_start as usize
        } else {
            token.delta_start as usize
        };
        let Some(Some(highlight)) = legend.types.get(token.token_type as usize) else {
            continue;
        };
        let start = Position::new(line, column);
        let end = Position::new(line, column + token.length as usize);
        let mutable = token.token_modifiers_bitset & legend.mutable != 0;
        spans.push((start..end, *highlight, mutable));
    }
    spans
}

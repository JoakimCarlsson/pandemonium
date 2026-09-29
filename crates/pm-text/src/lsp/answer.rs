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
    DocumentHighlightRequest, DocumentSymbolRequest, Formatting, GotoDeclaration, GotoDefinition,
    GotoImplementation, GotoTypeDefinition, HoverRequest, InlayHintRequest, References, Rename,
    ResolveCompletionItem, SemanticTokensFullRequest, SignatureHelpRequest, WillSaveWaitUntil,
    WorkspaceSymbolRequest,
};
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeLens,
    CodeLensParams, CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
    CompletionTextEdit, DocumentChangeOperation, DocumentChanges, DocumentFormattingParams,
    DocumentHighlightParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    Documentation, FormattingOptions, GotoDefinitionParams, GotoDefinitionResponse, HoverContents,
    HoverParams, InlayHint, InlayHintLabel, InlayHintParams, InsertTextFormat, MarkedString, OneOf,
    PartialResultParams, ReferenceContext, ReferenceParams, RenameParams, SemanticTokensParams,
    SemanticTokensResult, SignatureHelpParams, SymbolInformation, SymbolKind,
    TextDocumentIdentifier, TextDocumentPositionParams, TextDocumentSaveReason, TextEdit,
    WillSaveTextDocumentParams, WorkDoneProgressParams, WorkspaceEdit, WorkspaceSymbolParams,
    WorkspaceSymbolResponse,
};
use serde_json::Value;

use crate::cursor::Position;
use crate::indent::Indent;
use crate::lsp::capabilities::{Capabilities, Document};
use crate::lsp::encoding::Files;
use crate::lsp::{rpc, uri};
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
    /// What could be written here.
    Completions,
    /// The rest of what one of those completions says, for a server that
    /// sent it short.
    ResolveCompletion(Handle),
    /// The signature of the call this place is inside.
    Signature,
    /// The fixes and refactors the server offers here.
    CodeActions,
    /// Rename the symbol here to this everywhere it appears.
    Rename(String),
    /// Lay the whole file out the way the server's formatter would.
    Format,
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
    fn capability(&self) -> &'static str {
        match self {
            Self::Definition => "textDocument/definition",
            Self::TypeDefinition => "textDocument/typeDefinition",
            Self::Implementation => "textDocument/implementation",
            Self::Declaration => "textDocument/declaration",
            Self::References => "textDocument/references",
            Self::Hover => "textDocument/hover",
            Self::Completions | Self::ResolveCompletion(_) => "textDocument/completion",
            Self::Signature => "textDocument/signatureHelp",
            Self::CodeActions => "textDocument/codeAction",
            Self::Rename(_) => "textDocument/rename",
            Self::Format => "textDocument/formatting",
            Self::Symbols => "textDocument/documentSymbol",
            Self::Hints(_) => "textDocument/inlayHint",
            Self::Semantics => "textDocument/semanticTokens",
            Self::Occurrences => "textDocument/documentHighlight",
            Self::Lenses | Self::ResolveLens(_) => "textDocument/codeLens",
            Self::PrepareCalls(_) | Self::Calls(..) => "textDocument/prepareCallHierarchy",
            Self::WorkspaceSymbols(_) => "workspace/symbol",
            Self::WillSave => "textDocument/willSaveWaitUntil",
        }
    }

    /// The method this is asked by, for the log.
    pub(super) fn method(&self) -> &'static str {
        match self {
            Self::Semantics => "textDocument/semanticTokens/full",
            Self::ResolveCompletion(_) => "completionItem/resolve",
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
        if !capabilities.offers(self.capability(), document) {
            return false;
        }
        match self {
            Self::ResolveCompletion(_) => capabilities.resolves_completions(document),
            Self::ResolveLens(_) => capabilities.resolves_lenses(document),
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
            Self::Completions => rpc::request::<CompletionRequest>(
                id,
                CompletionParams {
                    text_document_position: place,
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                    context: None,
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
                    options: FormattingOptions {
                        tab_size: asking.indent.width as u32,
                        insert_spaces: !asking.indent.tabs,
                        ..FormattingOptions::default()
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
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
        })
    }

    /// What a server's reply to this request, made about `path`, comes to.
    ///
    /// `legend` is what the server said its token types are, in the order it
    /// numbers them; only a reply about semantics is read through it. A reply
    /// that is not the shape its method's result is comes to nothing.
    pub(super) fn read(
        &self,
        path: &Path,
        result: Value,
        legend: &[Option<Highlight>],
    ) -> Option<Answer> {
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
            Self::Completions => {
                Answer::Completions(completions(rpc::result::<CompletionRequest>(result)?))
            }
            Self::ResolveCompletion(_) => {
                let item = rpc::result::<ResolveCompletionItem>(result)?;
                Answer::Resolved(completion(item)?)
            }
            Self::Signature => Answer::Signature(
                rpc::result::<SignatureHelpRequest>(result)?
                    .and_then(|help| {
                        let active = help.active_signature.unwrap_or_default() as usize;
                        let mut signatures = help.signatures;
                        match active < signatures.len() {
                            true => Some(signatures.swap_remove(active).label),
                            false => signatures
                                .into_iter()
                                .next()
                                .map(|signature| signature.label),
                        }
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
            Self::Rename(_) => Answer::Edits(
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
                rpc::result::<SemanticTokensFullRequest>(result)?,
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
    /// What could be written where the cursor is.
    Completions(Vec<Completion>),
    /// One of those, filled in with what the server left out of the list.
    Resolved(Completion),
    /// The signature of the call the cursor is inside.
    Signature(String),
    /// The fixes and refactors offered where the cursor is.
    CodeActions(Vec<CodeAction>),
    /// Changes to make to files, from a rename or a formatter.
    Edits(Vec<FileEdit>),
    /// The symbols a file declares.
    Symbols(Vec<Symbol>),
    /// What the server would write into the lines it was asked about.
    Hints(Vec<crate::hint::Hint>),
    /// What the server makes of every name in the file, as spans to colour.
    Semantics(Vec<(Range<Position>, Highlight)>),
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
            Self::Hover(_) | Self::Signature(_) => {}
            Self::Completions(items) => {
                for item in items {
                    decode_completion(item, path, files);
                }
            }
            Self::Resolved(item) => decode_completion(item, path, files),
            Self::CodeActions(actions) => {
                for action in actions {
                    decode_edits(&mut action.edits, files);
                }
            }
            Self::Edits(edited) => decode_edits(edited, files),
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
                for (span, _) in spans {
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
            Self::Hover(text) | Self::Signature(text) => text.is_empty(),
            Self::Completions(items) => items.is_empty(),
            Self::Resolved(_) => false,
            Self::CodeActions(actions) => actions.is_empty(),
            Self::Edits(files) => files.iter().all(|file| file.edits.is_empty()),
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
    for (span, _) in &mut item.extra {
        *span = files.decode_span(path, span.clone());
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
    /// What kind of thing it is, as one word.
    pub kind: &'static str,
    /// The span it replaces, when the server named one.
    pub range: Option<Range<Position>>,
    /// Changes elsewhere in the file that choosing it brings along: the
    /// import a name needs, most often.
    pub extra: Vec<(Range<Position>, String)>,
    /// The server's own record of it, to have it filled in by.
    pub handle: Handle,
}

/// One fix or refactor the server offers.
#[derive(Clone, Debug)]
pub struct CodeAction {
    /// What the menu calls it.
    pub title: String,
    /// The changes it makes, when it carries them itself.
    pub edits: Vec<FileEdit>,
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
    let (span, text) = match item.text_edit.clone() {
        Some(CompletionTextEdit::Edit(edit)) => (Some(range(edit.range)), Some(edit.new_text)),
        Some(CompletionTextEdit::InsertAndReplace(edit)) => {
            (Some(range(edit.replace)), Some(edit.new_text))
        }
        None => (None, None),
    };
    let snippet = item.insert_text_format == Some(InsertTextFormat::SNIPPET);
    let insert = text
        .or_else(|| item.insert_text.clone())
        .unwrap_or_else(|| label.clone());
    Some(Completion {
        filter: item.filter_text.clone().unwrap_or_else(|| label.clone()),
        detail: item.detail.clone().unwrap_or_default(),
        documentation: match item.documentation.clone() {
            Some(Documentation::String(text)) => text,
            Some(Documentation::MarkupContent(markup)) => markup.value,
            None => String::new(),
        }
        .trim()
        .to_owned(),
        kind: item.kind.map_or("", completion_kind),
        insert: plain(&insert, snippet),
        range: span,
        extra: text_edits(item.additional_text_edits.clone().unwrap_or_default()),
        label,
        handle: Handle(Handed::Completion(Box::new(item))),
    })
}

/// A snippet written out as the plain text it would insert.
///
/// A snippet's placeholders are a second editing mode of their own; until
/// there is one, what goes in is the text with the placeholders taken out,
/// which is what the reader was going to type anyway.
fn plain(text: &str, snippet: bool) -> String {
    if !snippet {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.extend(chars.next()),
            '$' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut depth = 1;
                let mut body = String::new();
                for ch in chars.by_ref() {
                    match ch {
                        '{' => depth += 1,
                        '}' if depth == 1 => break,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    body.push(ch);
                }
                if let Some((_, rest)) = body.split_once(':') {
                    out.push_str(rest);
                }
            }
            '$' => {
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
            }
            ch => out.push(ch),
        }
    }
    out
}

/// What a completion's kind is called.
fn completion_kind(kind: CompletionItemKind) -> &'static str {
    match kind {
        CompletionItemKind::METHOD => "method",
        CompletionItemKind::FUNCTION => "function",
        CompletionItemKind::CONSTRUCTOR => "constructor",
        CompletionItemKind::FIELD => "field",
        CompletionItemKind::VARIABLE => "variable",
        CompletionItemKind::CLASS => "class",
        CompletionItemKind::INTERFACE => "interface",
        CompletionItemKind::MODULE => "module",
        CompletionItemKind::PROPERTY => "property",
        CompletionItemKind::ENUM => "enum",
        CompletionItemKind::KEYWORD => "keyword",
        CompletionItemKind::SNIPPET => "snippet",
        CompletionItemKind::CONSTANT => "constant",
        CompletionItemKind::STRUCT => "struct",
        CompletionItemKind::EVENT => "event",
        CompletionItemKind::TYPE_PARAMETER => "type",
        _ => "",
    }
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

/// The changes a workspace edit asks for, file by file.
pub(super) fn workspace_edit(edit: &WorkspaceEdit) -> Vec<FileEdit> {
    let mut files = Vec::new();
    for (uri, edits) in edit.changes.iter().flatten() {
        if let Some(path) = uri::path_of(uri) {
            files.push(FileEdit {
                path,
                edits: text_edits(edits.clone()),
            });
        }
    }
    let documents = match &edit.document_changes {
        None => Vec::new(),
        Some(DocumentChanges::Edits(edits)) => edits.iter().collect(),
        Some(DocumentChanges::Operations(operations)) => operations
            .iter()
            .filter_map(|operation| match operation {
                DocumentChangeOperation::Edit(edit) => Some(edit),
                DocumentChangeOperation::Op(_) => None,
            })
            .collect(),
    };
    for document in documents {
        if let Some(path) = uri::path_of(&document.text_document.uri) {
            files.push(FileEdit {
                path,
                edits: document
                    .edits
                    .iter()
                    .map(|edit| match edit {
                        OneOf::Left(edit) => edit,
                        OneOf::Right(annotated) => &annotated.text_edit,
                    })
                    .map(|edit| (range(edit.range), edit.new_text.clone()))
                    .collect(),
            });
        }
    }
    files
}

/// Whether the editor can make every change `edit` asks for.
///
/// Files are changed, not made, moved or removed: an edit that asks for
/// any of those is one the editor would only make part of.
pub(super) fn is_supported(edit: &WorkspaceEdit) -> bool {
    let operations = match &edit.document_changes {
        Some(DocumentChanges::Operations(operations)) => operations.as_slice(),
        _ => &[],
    };
    let changes = edit
        .changes
        .iter()
        .flatten()
        .all(|(uri, _)| uri::path_of(uri).is_some());
    let documents = match &edit.document_changes {
        Some(DocumentChanges::Edits(edits)) => edits
            .iter()
            .all(|edit| uri::path_of(&edit.text_document.uri).is_some()),
        _ => true,
    };
    let operations = operations.iter().all(|operation| match operation {
        DocumentChangeOperation::Edit(edit) => uri::path_of(&edit.text_document.uri).is_some(),
        DocumentChangeOperation::Op(_) => false,
    });
    (edit.changes.is_some() || edit.document_changes.is_some())
        && changes
        && documents
        && operations
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

/// The legend a server publishes, as the highlight each of its types means.
pub(super) fn legend(capabilities: &Capabilities) -> Vec<Option<Highlight>> {
    capabilities
        .legend()
        .map(|legend| {
            legend
                .token_types
                .iter()
                .map(|kind| Highlight::of_token(kind.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

/// The spans a server's semantic tokens come to, in the file's own terms.
///
/// The protocol sends them relative to the token before each: a line down
/// from the last token's, a column along from it when they share a line, a
/// length, a type and its modifiers. A token whose type the editor draws no
/// differently is left out here rather than carried to the painter to be
/// discarded there.
fn semantics(
    found: Option<SemanticTokensResult>,
    legend: &[Option<Highlight>],
) -> Vec<(Range<Position>, Highlight)> {
    let tokens = match found {
        None => return Vec::new(),
        Some(SemanticTokensResult::Tokens(tokens)) => tokens.data,
        Some(SemanticTokensResult::Partial(partial)) => partial.data,
    };
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
        let Some(Some(highlight)) = legend.get(token.token_type as usize) else {
            continue;
        };
        let start = Position::new(line, column);
        let end = Position::new(line, column + token.length as usize);
        spans.push((start..end, *highlight));
    }
    spans
}

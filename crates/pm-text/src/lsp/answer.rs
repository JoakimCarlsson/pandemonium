//! What a language server can be asked, and what it says back.
//!
//! The protocol's own shapes stop here. A request is one of a closed set of
//! questions, an answer is one of a closed set of replies, and both are in
//! the editor's own terms — paths and [`Position`]s — so nothing above this
//! layer ever sees a `file://` URI or a JSON value.

use std::ops::Range;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::cursor::Position;
use crate::indent::Indent;
use crate::lsp::encoding::Files;
use crate::lsp::uri;
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
    /// The symbol here, as something whose calls can be followed.
    PrepareCalls(Calls),
    /// The calls into or out of a symbol the server named.
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
/// A code lens to resolve and a symbol whose calls are to be followed are
/// both the server's own records: the editor keeps them without reading
/// them and returns them to the server that made them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Handle(Value);

impl Request {
    /// The method a server is asked by.
    pub(super) fn method(&self) -> &'static str {
        match self {
            Self::Definition => "textDocument/definition",
            Self::TypeDefinition => "textDocument/typeDefinition",
            Self::Implementation => "textDocument/implementation",
            Self::Declaration => "textDocument/declaration",
            Self::References => "textDocument/references",
            Self::Hover => "textDocument/hover",
            Self::Completions => "textDocument/completion",
            Self::Signature => "textDocument/signatureHelp",
            Self::CodeActions => "textDocument/codeAction",
            Self::Rename(_) => "textDocument/rename",
            Self::Format => "textDocument/formatting",
            Self::Symbols => "textDocument/documentSymbol",
            Self::Hints(_) => "textDocument/inlayHint",
            Self::Semantics => "textDocument/semanticTokens/full",
            Self::Occurrences => "textDocument/documentHighlight",
            Self::Lenses => "textDocument/codeLens",
            Self::ResolveLens(_) => "codeLens/resolve",
            Self::PrepareCalls(_) => "textDocument/prepareCallHierarchy",
            Self::Calls(Calls::Incoming, _) => "callHierarchy/incomingCalls",
            Self::Calls(Calls::Outgoing, _) => "callHierarchy/outgoingCalls",
            Self::WorkspaceSymbols(_) => "workspace/symbol",
            Self::WillSave => "textDocument/willSaveWaitUntil",
        }
    }

    /// Whether a server that declared `capabilities` answers this request.
    ///
    /// A server asked what it never offered answers with an error at best,
    /// and a save that waits on the answer waits on a refusal.
    pub(super) fn is_offered(&self, capabilities: &Value) -> bool {
        let offered = match self {
            Self::Definition => &capabilities["definitionProvider"],
            Self::TypeDefinition => &capabilities["typeDefinitionProvider"],
            Self::Implementation => &capabilities["implementationProvider"],
            Self::Declaration => &capabilities["declarationProvider"],
            Self::References => &capabilities["referencesProvider"],
            Self::Hover => &capabilities["hoverProvider"],
            Self::Completions => &capabilities["completionProvider"],
            Self::Signature => &capabilities["signatureHelpProvider"],
            Self::CodeActions => &capabilities["codeActionProvider"],
            Self::Rename(_) => &capabilities["renameProvider"],
            Self::Format => &capabilities["documentFormattingProvider"],
            Self::Symbols => &capabilities["documentSymbolProvider"],
            Self::Hints(_) => &capabilities["inlayHintProvider"],
            Self::Semantics => &capabilities["semanticTokensProvider"],
            Self::Occurrences => &capabilities["documentHighlightProvider"],
            Self::Lenses => &capabilities["codeLensProvider"],
            Self::ResolveLens(_) => &capabilities["codeLensProvider"]["resolveProvider"],
            Self::PrepareCalls(_) | Self::Calls(..) => &capabilities["callHierarchyProvider"],
            Self::WorkspaceSymbols(_) => &capabilities["workspaceSymbolProvider"],
            Self::WillSave => &capabilities["textDocumentSync"]["willSaveWaitUntil"],
        };
        !matches!(offered, Value::Null | Value::Bool(false))
    }

    /// This request with every place named in it counted the server's way.
    ///
    /// Only a request carrying a span of its own has anything to translate;
    /// the place the rest are asked about is translated by the client as it
    /// asks them.
    pub(super) fn encoded(&self, path: &std::path::Path, files: &mut Files) -> Self {
        match self {
            Self::Hints(span) => {
                Self::Hints(files.encode(path, span.start)..files.encode(path, span.end))
            }
            request => request.clone(),
        }
    }

    /// The parameters it is asked with, about `at` in the file at `path`.
    pub(super) fn params(
        &self,
        path: &std::path::Path,
        at: Position,
        indent: Indent,
        selection: Range<Position>,
        diagnostics: Vec<Value>,
    ) -> Value {
        let document = json!({ "uri": uri::of(path) });
        let position = json!({ "line": at.line, "character": at.column });

        match self {
            Self::References => json!({
                "textDocument": document,
                "position": position,
                "context": { "includeDeclaration": false },
            }),
            Self::Rename(name) => json!({
                "textDocument": document,
                "position": position,
                "newName": name,
            }),
            Self::CodeActions => json!({
                "textDocument": document,
                "range": {
                    "start": { "line": selection.start.line, "character": selection.start.column },
                    "end": { "line": selection.end.line, "character": selection.end.column },
                },
                "context": { "diagnostics": diagnostics },
            }),
            Self::Format => json!({
                "textDocument": document,
                "options": { "tabSize": indent.width, "insertSpaces": !indent.tabs },
            }),
            Self::Symbols | Self::Semantics | Self::Lenses => json!({ "textDocument": document }),
            Self::ResolveLens(Handle(lens)) => lens.clone(),
            Self::Calls(_, Handle(item)) => json!({ "item": item }),
            Self::WorkspaceSymbols(query) => json!({ "query": query }),
            Self::WillSave => json!({ "textDocument": document, "reason": 1 }),
            Self::Hints(span) => json!({
                "textDocument": document,
                "range": {
                    "start": { "line": span.start.line, "character": span.start.column },
                    "end": { "line": span.end.line, "character": span.end.column },
                },
            }),
            _ => json!({ "textDocument": document, "position": position }),
        }
    }

    /// What a server's reply to this request, made about `path`, comes to.
    ///
    /// `legend` is what the server said its token types are, in the order it
    /// numbers them; only a reply about semantics is read through it.
    pub(super) fn read(
        &self,
        path: &std::path::Path,
        result: &Value,
        legend: &[Option<Highlight>],
    ) -> Answer {
        match self {
            Self::Definition
            | Self::TypeDefinition
            | Self::Implementation
            | Self::Declaration
            | Self::References => Answer::Locations(locations(result)),
            Self::Hover => Answer::Hover(hover(result)),
            Self::Completions => Answer::Completions(completions(result)),
            Self::Signature => Answer::Signature(signature(result)),
            Self::CodeActions => Answer::CodeActions(code_actions(result)),
            Self::Rename(_) => Answer::Edits(workspace_edit(result)),
            Self::Format => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: text_edits(result),
            }]),
            Self::Symbols => Answer::Symbols(symbols(result)),
            Self::Hints(_) => Answer::Hints(hints(result)),
            Self::Semantics => Answer::Semantics(semantics(result, legend)),
            Self::Occurrences => Answer::Occurrences(spans(result)),
            Self::Lenses => Answer::Lenses(lenses(result)),
            Self::ResolveLens(_) => Answer::Lenses(lens(result).into_iter().collect()),
            Self::PrepareCalls(_) => Answer::CallItems(
                result
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Handle)
                    .collect(),
            ),
            Self::Calls(direction, _) => Answer::Named(calls(*direction, result)),
            Self::WorkspaceSymbols(_) => Answer::Named(workspace_symbols(result)),
            Self::WillSave => Answer::Edits(vec![FileEdit {
                path: path.to_path_buf(),
                edits: text_edits(result),
            }]),
        }
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
    pub(super) fn decode(&mut self, path: &std::path::Path, files: &mut Files) {
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
                    item.range = item.range.clone().map(|span| files.decode_span(path, span));
                }
            }
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
    /// What is said beside it: a type, a signature, a module.
    pub detail: String,
    /// What goes into the buffer when it is chosen.
    pub insert: String,
    /// What kind of thing it is, as one word.
    pub kind: &'static str,
    /// The span it replaces, when the server named one.
    pub range: Option<Range<Position>>,
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
    /// The arguments returned by the server.
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
fn position(value: &Value) -> Position {
    Position::new(
        value["line"].as_u64().unwrap_or_default() as usize,
        value["character"].as_u64().unwrap_or_default() as usize,
    )
}

/// One span, in the editor's own terms.
fn range(value: &Value) -> Range<Position> {
    position(&value["start"])..position(&value["end"])
}

/// The spans a list of ranged things covers, whatever else each says.
fn spans(result: &Value) -> Vec<Range<Position>> {
    result
        .as_array()
        .map(|found| found.iter().map(|each| range(&each["range"])).collect())
        .unwrap_or_default()
}

/// The notes a server would put above a file's declarations.
fn lenses(result: &Value) -> Vec<Lens> {
    result
        .as_array()
        .map(|found| found.iter().filter_map(lens).collect())
        .unwrap_or_default()
}

/// One note, said or waiting to be resolved.
fn lens(value: &Value) -> Option<Lens> {
    value.get("range")?;
    Some(Lens {
        position: position(&value["range"]["start"]),
        title: value["command"]["title"].as_str().map(str::to_owned),
        handle: Handle(value.clone()),
    })
}

/// The symbol a call hierarchy item names, and where it is.
fn call_item(item: &Value, span: &Value) -> Option<NamedLocation> {
    Some(NamedLocation {
        name: item["name"].as_str()?.to_owned(),
        detail: item["detail"].as_str().unwrap_or_default().to_owned(),
        kind: symbol_kind(item["kind"].as_u64().unwrap_or_default()),
        location: Location {
            path: uri::path(item["uri"].as_str()?)?,
            range: range(span),
            origin: None,
        },
    })
}

/// The calls into or out of a symbol, each as the symbol at its other end.
///
/// A caller is shown where it makes the call, which is what a reader
/// following calls upward wants to read; a callee is shown where it is
/// declared, since the call itself is in the file already open.
fn calls(direction: Calls, result: &Value) -> Vec<NamedLocation> {
    result
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|call| match direction {
            Calls::Incoming => {
                let item = &call["from"];
                let span = call["fromRanges"].get(0).unwrap_or(&item["selectionRange"]);
                call_item(item, span)
            }
            Calls::Outgoing => call_item(&call["to"], &call["to"]["selectionRange"]),
        })
        .collect()
}

/// The symbols of a workspace a query matched, in the order the server ranked them.
///
/// A symbol whose location names only its file, which a server may send to
/// have it resolved later, is taken to be at the top of that file.
fn workspace_symbols(result: &Value) -> Vec<NamedLocation> {
    result
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|symbol| {
            let location = &symbol["location"];
            Some(NamedLocation {
                name: symbol["name"].as_str()?.to_owned(),
                detail: symbol["containerName"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                kind: symbol_kind(symbol["kind"].as_u64().unwrap_or_default()),
                location: Location {
                    path: uri::path(location["uri"].as_str()?)?,
                    range: range(&location["range"]),
                    origin: None,
                },
            })
        })
        .collect()
}

/// The places a location, a link or a list of either names.
fn locations(result: &Value) -> Vec<Location> {
    let values = match result {
        Value::Array(values) => values.clone(),
        Value::Null => return Vec::new(),
        value => vec![value.clone()],
    };

    values
        .iter()
        .filter_map(|value| {
            let uri = value["uri"]
                .as_str()
                .or_else(|| value["targetUri"].as_str())?;
            let span = if value.get("targetSelectionRange").is_some() {
                &value["targetSelectionRange"]
            } else if value.get("targetRange").is_some() {
                &value["targetRange"]
            } else {
                &value["range"]
            };
            Some(Location {
                path: uri::path(uri)?,
                range: range(span),
                origin: value.get("originSelectionRange").map(range),
            })
        })
        .collect()
}

/// What a hover says, with the protocol's wrappers taken off.
fn hover(result: &Value) -> String {
    let contents = &result["contents"];
    let text = match contents {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|part| match part {
                Value::String(text) => text.clone(),
                part => part["value"].as_str().unwrap_or_default().to_owned(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        part => part["value"].as_str().unwrap_or_default().to_owned(),
    };
    text.trim().to_owned()
}

/// What could be written, in the order the server ranked it.
fn completions(result: &Value) -> Vec<Completion> {
    let items = match result {
        Value::Array(items) => items.clone(),
        value => value["items"].as_array().cloned().unwrap_or_default(),
    };

    items
        .iter()
        .filter_map(|item| {
            let label = item["label"].as_str()?.trim().to_owned();
            let edit = &item["textEdit"];
            let span = edit
                .get("range")
                .map(range)
                .or_else(|| edit.get("replace").map(range));
            let insert = edit["newText"]
                .as_str()
                .or_else(|| item["insertText"].as_str())
                .unwrap_or(&label)
                .to_owned();
            Some(Completion {
                detail: item["detail"].as_str().unwrap_or_default().to_owned(),
                kind: completion_kind(item["kind"].as_u64().unwrap_or_default()),
                insert: plain(&insert, item["insertTextFormat"].as_u64() == Some(2)),
                range: span,
                label,
            })
        })
        .collect()
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

/// What a completion's numeric kind is called.
fn completion_kind(kind: u64) -> &'static str {
    match kind {
        2 => "method",
        3 => "function",
        4 => "constructor",
        5 => "field",
        6 => "variable",
        7 => "class",
        8 => "interface",
        9 => "module",
        10 => "property",
        13 => "enum",
        14 => "keyword",
        15 => "snippet",
        21 => "constant",
        22 => "struct",
        23 => "event",
        25 => "type",
        _ => "",
    }
}

/// The signature the server is offering, as one line.
fn signature(result: &Value) -> String {
    let signatures = result["signatures"].as_array().cloned().unwrap_or_default();
    let active = result["activeSignature"].as_u64().unwrap_or_default() as usize;
    signatures
        .get(active)
        .or_else(|| signatures.first())
        .and_then(|signature| signature["label"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// The fixes and refactors offered, including commands without edits.
fn code_actions(result: &Value) -> Vec<CodeAction> {
    result
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|action| {
            let title = action["title"].as_str()?.to_owned();
            let edits = workspace_edit(&action["edit"]);
            let command = if action["command"].is_string() {
                command(action)
            } else {
                command(&action["command"])
            };
            (!edits.is_empty() || command.is_some()).then_some(CodeAction {
                title,
                edits,
                command,
            })
        })
        .collect()
}

/// The executable part of a code action or a command-only action.
fn command(value: &Value) -> Option<Command> {
    Some(Command {
        name: value["command"].as_str()?.to_owned(),
        arguments: value["arguments"].as_array().cloned().unwrap_or_default(),
    })
}

/// The changes a workspace edit asks for, file by file.
pub(super) fn workspace_edit(edit: &Value) -> Vec<FileEdit> {
    let mut files = Vec::new();

    if let Some(changes) = edit["changes"].as_object() {
        for (uri, edits) in changes {
            if let Some(path) = uri::path(uri) {
                files.push(FileEdit {
                    path,
                    edits: text_edits(edits),
                });
            }
        }
    }

    for change in edit["documentChanges"].as_array().unwrap_or(&Vec::new()) {
        let Some(uri) = change["textDocument"]["uri"].as_str() else {
            continue;
        };
        if let Some(path) = uri::path(uri) {
            files.push(FileEdit {
                path,
                edits: text_edits(&change["edits"]),
            });
        }
    }
    files
}

/// The spans one file is asked to replace, and what with.
pub(super) fn text_edits(edits: &Value) -> Vec<(Range<Position>, String)> {
    edits
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|edit| {
            (
                range(&edit["range"]),
                edit["newText"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// What a server would write into the lines, in the editor's own terms.
///
/// A hint's label is either a string or a run of parts, each of which may
/// carry a link back into the source; what is drawn is the text of them,
/// because a hint is a note in the margin of a line and not a control.
fn hints(result: &Value) -> Vec<crate::hint::Hint> {
    result
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|hint| {
            let text = match &hint["label"] {
                Value::String(text) => text.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .map(|part| part["value"].as_str().unwrap_or_default())
                    .collect(),
                _ => return None,
            };
            let padded = format!(
                "{}{}{}",
                if hint["paddingLeft"] == json!(true) {
                    " "
                } else {
                    ""
                },
                text.trim(),
                if hint["paddingRight"] == json!(true) {
                    " "
                } else {
                    ""
                },
            );
            (!padded.trim().is_empty()).then(|| crate::hint::Hint {
                position: position(&hint["position"]),
                text: padded,
            })
        })
        .collect()
}

/// The symbols a file declares, flattened in the order they appear.
fn symbols(result: &Value) -> Vec<Symbol> {
    let mut found = Vec::new();
    collect_symbols(result, 0, &mut found);
    found
}

/// Walks one level of the symbol tree, and the levels under it.
fn collect_symbols(value: &Value, depth: usize, found: &mut Vec<Symbol>) {
    for symbol in value.as_array().unwrap_or(&Vec::new()) {
        let Some(name) = symbol["name"].as_str() else {
            continue;
        };
        let at = if symbol.get("selectionRange").is_some() {
            position(&symbol["selectionRange"]["start"])
        } else {
            position(&symbol["location"]["range"]["start"])
        };
        found.push(Symbol {
            name: name.to_owned(),
            detail: symbol["detail"].as_str().unwrap_or_default().to_owned(),
            kind: symbol_kind(symbol["kind"].as_u64().unwrap_or_default()),
            position: at,
            depth,
        });
        collect_symbols(&symbol["children"], depth + 1, found);
    }
}

/// What a symbol's numeric kind is called.
fn symbol_kind(kind: u64) -> &'static str {
    match kind {
        2 => "module",
        5 => "class",
        6 => "method",
        7 => "property",
        8 => "field",
        9 => "constructor",
        10 => "enum",
        11 => "interface",
        12 => "function",
        13 => "variable",
        14 => "constant",
        23 => "struct",
        26 => "type",
        _ => "",
    }
}

/// The legend a server publishes, as the highlight each of its types means.
pub(super) fn legend(capabilities: &Value) -> Vec<Option<Highlight>> {
    capabilities["semanticTokensProvider"]["legend"]["tokenTypes"]
        .as_array()
        .map(|types| {
            types
                .iter()
                .map(|kind| kind.as_str().and_then(Highlight::of_token))
                .collect()
        })
        .unwrap_or_default()
}

/// The spans a server's semantic tokens come to, in the file's own terms.
///
/// The protocol sends them as five numbers each, every one of them relative
/// to the token before it: a line down from the last token's, a column along
/// from it when they share a line, a length, a type and its modifiers. A
/// token whose type the editor draws no differently is left out here rather
/// than carried to the painter to be discarded there.
fn semantics(result: &Value, legend: &[Option<Highlight>]) -> Vec<(Range<Position>, Highlight)> {
    const STRIDE: usize = 5;

    let Some(data) = result["data"].as_array() else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let (mut line, mut column) = (0_usize, 0_usize);

    for token in data.as_chunks::<STRIDE>().0 {
        let [down, along, length, kind, _] =
            std::array::from_fn(|index| token[index].as_u64().unwrap_or_default() as usize);

        line += down;
        column = if down == 0 { column + along } else { along };

        let Some(Some(highlight)) = legend.get(kind) else {
            continue;
        };
        let start = Position::new(line, column);
        let end = Position::new(line, column + length);
        spans.push((start..end, *highlight));
    }
    spans
}

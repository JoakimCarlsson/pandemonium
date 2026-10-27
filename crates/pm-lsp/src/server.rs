//! One running language server: started for a project, told of the files open in it, and asked for completions and hovers.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pm_buffer::{Position, Span};
use serde_json::{Value, json};

use crate::manifest::ServerSpec;
use crate::resolve::{found, managed, resolved};
use crate::rpc::{Handler, Rpc};
use crate::snippet;
use crate::types::{
    CodeAction, CodeLens, Completion, CompletionKind, Diagnostic, FoldRange, HintKind, Hover,
    InlayHint, InlineValue, InlineValueKind, Seek, SemanticToken, Severity, Signature, TextChange,
    TokenKind,
};
use crate::uri::file_uri;

/// How long a server may take to start.
const STARTING: Duration = Duration::from_secs(60);

/// How long a server may take to answer a question about a file.
const ANSWERING: Duration = Duration::from_secs(10);

/// How long changes to a file must stop coming before a server that gives diagnostics when asked is asked for them.
const PULL_AFTER: Duration = Duration::from_millis(300);

/// How long a server is given to stop before its process is ended.
const STOPPING: Duration = Duration::from_secs(2);

/// What is done with the diagnostics a server publishes: the address of the file, the version they are about and the diagnostics.
pub(crate) type DiagnosticsSink = Arc<dyn Fn(String, Option<i32>, Vec<Diagnostic>) + Send + Sync>;

/// What a server last said is wrong with each file, as it wrote it, by address, to hand back when asking it for changes at a place.
type Raw = Arc<Mutex<HashMap<String, Vec<Value>>>>;

/// The edits a server asked to have made while it carried out a command, as it wrote them.
type Applied = Arc<Mutex<Vec<Value>>>;

/// A place a server names: the address of the file, the stretch, and what the place is called when it says.
pub(crate) type Found = (String, Span, Option<String>);

/// What pressing a code lens came to.
pub(crate) enum Pressed {
    /// Places to show, as for the lens that counts the uses of a name.
    Places(Vec<Found>),
    /// Edits the server asked to have made, as it wrote them.
    Edits(Vec<Value>),
}

/// The kinds of names a client says it understands, in the order of the legend a server answers with its own.
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

/// The most hints a client is given for one file.
const MOST_HINTS: usize = 4000;

/// The most lenses a client is given for one file, and the most a server is asked to fill in.
const MOST_LENSES: usize = 300;

/// A running language server.
pub(crate) struct LanguageServer {
    /// The conversation with it.
    rpc: Arc<Rpc>,
    /// What it can do, as it said when it started.
    abilities: Value,
    /// What it last said is wrong with each file, as it wrote it.
    raw: Raw,
    /// The edits it asked to have made while carrying out a command.
    applied: Applied,
    /// How it takes changes: 0 not at all, 1 as the whole text, 2 as stretches.
    sync: u64,
    /// The characters after which it offers completions without being asked.
    triggers: Vec<String>,
    /// Where the diagnostics it is asked for go, as the ones it publishes do.
    sink: DiagnosticsSink,
    /// For each file it is asked diagnostics of, by address: how many times it was asked for, to leave all but the last of a burst of changes unasked, and the result it last answered with.
    pulled: Mutex<HashMap<String, (u64, Option<String>)>>,
}

/// The value at the dotted path `section` of `settings`, or nothing.
fn section_of(settings: &Value, section: Option<&str>) -> Value {
    let Some(section) = section.filter(|section| !section.is_empty()) else {
        return settings.clone();
    };
    section
        .split('.')
        .try_fold(settings, |held, key| held.get(key))
        .cloned()
        .unwrap_or(Value::Null)
}

/// The span a language server writes as `range`.
fn span_of(range: &Value) -> Option<Span> {
    let position = |value: &Value| {
        Some(Position::new(
            value.get("line")?.as_u64()? as u32,
            value.get("character")?.as_u64()? as u32,
        ))
    };
    Some(Span::new(
        position(range.get("start")?)?,
        position(range.get("end")?)?,
    ))
}

/// `position` as a language server writes it.
fn position_value(position: Position) -> Value {
    json!({"line": position.line, "character": position.character})
}

/// `span` as a language server writes it.
fn range_value(span: Span) -> Value {
    json!({"start": position_value(span.start), "end": position_value(span.end)})
}

/// The diagnostic a server writes as `value`.
fn diagnostic_of(value: &Value) -> Option<Diagnostic> {
    Some(Diagnostic {
        span: span_of(value.get("range")?)?,
        severity: Severity::of_number(value.get("severity").and_then(Value::as_u64)),
        message: value.get("message")?.as_str()?.to_owned(),
        source: value
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// The text a server writes as a marked string or markup: a string, a block of code in a language, or markdown.
fn markup_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(markup_of)
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Value::Object(_) => {
            let text = value
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match value.get("language").and_then(Value::as_str) {
                Some(language) => format!("```{language}\n{text}\n```"),
                None => text.to_owned(),
            }
        }
        _ => String::new(),
    }
}

/// The completion a server writes as `item`, the stretch it replaces being `default_span` when it names none.
fn completion_of(item: &Value, default_span: Option<Span>) -> Option<Completion> {
    let label = item.get("label")?.as_str()?.to_owned();
    let edit = item.get("textEdit");
    let raw = edit
        .and_then(|edit| edit.get("newText"))
        .or_else(|| item.get("insertText"))
        .and_then(Value::as_str)
        .unwrap_or(&label)
        .to_owned();
    let text = if item.get("insertTextFormat").and_then(Value::as_u64) == Some(2) {
        snippet::plain(&raw)
    } else {
        raw
    };
    let span = edit
        .and_then(|edit| edit.get("range").or_else(|| edit.get("insert")))
        .and_then(span_of)
        .or(default_span);
    let detail = item
        .get("detail")
        .and_then(Value::as_str)
        .or_else(|| {
            item.get("labelDetails")
                .and_then(|details| details.get("description").or_else(|| details.get("detail")))
                .and_then(Value::as_str)
        })
        .map(|detail| detail.trim().to_owned())
        .filter(|detail| !detail.is_empty());
    Some(Completion {
        filter: item
            .get("filterText")
            .and_then(Value::as_str)
            .unwrap_or(&label)
            .to_owned(),
        sort: item
            .get("sortText")
            .and_then(Value::as_str)
            .unwrap_or(&label)
            .to_owned(),
        kind: CompletionKind::of_number(item.get("kind").and_then(Value::as_u64)),
        documentation: item
            .get("documentation")
            .map(markup_of)
            .filter(|said| !said.trim().is_empty()),
        label,
        detail,
        text,
        span,
    })
}

/// What this client can do, as it tells a server it starts.
fn capabilities() -> Value {
    json!({
        "general": {"positionEncodings": ["utf-16"]},
        "experimental": {"commands": {"commands": ["rust-analyzer.showReferences"]}},
        "workspace": {
            "configuration": true,
            "workspaceFolders": true,
            "applyEdit": true,
            "workspaceEdit": {"documentChanges": true},
            "executeCommand": {},
            "inlayHint": {"refreshSupport": false},
            "codeLens": {"refreshSupport": false},
            "semanticTokens": {"refreshSupport": false},
            "fileOperations": {"willRename": true, "didRename": true}
        },
        "textDocument": {
            "definition": {"linkSupport": true},
            "declaration": {"linkSupport": true},
            "typeDefinition": {"linkSupport": true},
            "implementation": {"linkSupport": true},
            "references": {},
            "documentHighlight": {},
            "signatureHelp": {
                "signatureInformation": {
                    "documentationFormat": ["markdown", "plaintext"],
                    "parameterInformation": {"labelOffsetSupport": true},
                    "activeParameterSupport": true
                },
                "contextSupport": true
            },
            "callHierarchy": {},
            "typeHierarchy": {},
            "semanticTokens": {
                "requests": {"range": false, "full": true},
                "tokenTypes": TOKEN_TYPES,
                "tokenModifiers": ["declaration", "definition", "readonly", "static", "deprecated", "abstract", "async", "modification", "documentation", "defaultLibrary"],
                "formats": ["relative"],
                "overlappingTokenSupport": false,
                "multilineTokenSupport": false
            },
            "inlayHint": {"dynamicRegistration": false},
            "inlineValue": {"dynamicRegistration": false},
            "codeLens": {"dynamicRegistration": false},
            "rename": {"prepareSupport": false},
            "codeAction": {
                "codeActionLiteralSupport": {
                    "codeActionKind": {"valueSet": [
                        "", "quickfix", "refactor", "refactor.extract", "refactor.inline",
                        "refactor.rewrite", "source", "source.organizeImports", "source.fixAll"
                    ]}
                },
                "isPreferredSupport": true,
                "disabledSupport": true,
                "dataSupport": true,
                "resolveSupport": {"properties": ["edit"]}
            },
            "synchronization": {"dynamicRegistration": false, "didSave": false},
            "completion": {
                "completionItem": {
                    "snippetSupport": true,
                    "documentationFormat": ["markdown", "plaintext"],
                    "labelDetailsSupport": true
                },
                "contextSupport": true
            },
            "hover": {"contentFormat": ["markdown", "plaintext"]},
            "formatting": {},
            "rangeFormatting": {},
            "onTypeFormatting": {},
            "foldingRange": {"lineFoldingOnly": false},
            "linkedEditingRange": {},
            "publishDiagnostics": {"versionSupport": true},
            "diagnostic": {"dynamicRegistration": false, "relatedDocumentSupport": false}
        },
        "window": {"workDoneProgress": false}
    })
}

/// What answers the requests of a server started for `root` with `settings`, hands the diagnostics it publishes to `sink` and keeps them as written in `raw`, and keeps the edits it asks to have made in `applied`.
fn handler(
    root: &Path,
    settings: Value,
    sink: DiagnosticsSink,
    raw: Raw,
    applied: Applied,
) -> Handler {
    let folders = json!([{
        "uri": file_uri(root),
        "name": root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
    }]);
    Arc::new(move |method, params| match method {
        "workspace/configuration" => Value::Array(
            params
                .get("items")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|item| {
                            section_of(&settings, item.get("section").and_then(Value::as_str))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        ),
        "workspace/workspaceFolders" => folders.clone(),
        "workspace/applyEdit" => {
            if let (Some(edit), Ok(mut held)) = (params.get("edit"), applied.lock()) {
                held.push(edit.clone());
            }
            json!({"applied": true})
        }
        "textDocument/publishDiagnostics" => {
            if let Some(uri) = params.get("uri").and_then(Value::as_str) {
                if let (Some(list), Ok(mut held)) = (
                    params.get("diagnostics").and_then(Value::as_array),
                    raw.lock(),
                ) {
                    held.insert(uri.to_owned(), list.clone());
                }
                let diagnostics = params
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(diagnostic_of).collect())
                    .unwrap_or_default();
                let version = params
                    .get("version")
                    .and_then(Value::as_i64)
                    .map(|version| version as i32);
                sink(uri.to_owned(), version, diagnostics);
            }
            Value::Null
        }
        _ => Value::Null,
    })
}

impl LanguageServer {
    /// Starts the server of `spec` for the project at `root`, handing what it says is wrong with files to `sink`: the project's or the machine's own program first, and the one installed in the data folder when there is none or it does not start, installing that first when it is missing.
    pub(crate) fn start(
        spec: &ServerSpec,
        root: &Path,
        env: &BTreeMap<String, String>,
        sink: DiagnosticsSink,
    ) -> Result<Self, String> {
        if let Some(chosen) = resolved(spec, root) {
            let chosen = chosen?;
            let mut with = spec.clone();
            if !chosen.args.is_empty() {
                with.args = chosen.args;
            }
            let mut environment = env.clone();
            environment.extend(chosen.env);
            return Self::start_program(&chosen.program, &with, root, &environment, sink);
        }
        let first_try = match found(spec, Some(root)) {
            Some(program) => match Self::start_program(&program, spec, root, env, sink.clone()) {
                Ok(server) => return Ok(server),
                Err(reason) if spec.install.is_some() => Some(reason),
                Err(reason) => return Err(reason),
            },
            None => None,
        };
        let program = managed(spec).map_err(|reason| match &first_try {
            Some(before) => format!("{before}; {reason}"),
            None => reason,
        })?;
        Self::start_program(&program, spec, root, env, sink)
    }

    /// Starts `program` as the server of `spec` for the project at `root` and sets it up.
    fn start_program(
        program: &Path,
        spec: &ServerSpec,
        root: &Path,
        env: &BTreeMap<String, String>,
        sink: DiagnosticsSink,
    ) -> Result<Self, String> {
        let first: Vec<PathBuf> = spec
            .project
            .iter()
            .map(|folder| root.join(folder))
            .collect();
        let raw = Raw::default();
        let applied = Applied::default();
        let rpc = Rpc::start(
            program,
            &spec.args,
            root,
            &first,
            env,
            handler(
                root,
                spec.settings.clone(),
                sink.clone(),
                raw.clone(),
                applied.clone(),
            ),
        )?;
        let mut params = json!({
            "processId": std::process::id(),
            "clientInfo": {"name": "pandemonium", "version": env!("CARGO_PKG_VERSION")},
            "rootUri": file_uri(root),
            "rootPath": root.to_string_lossy(),
            "workspaceFolders": [{
                "uri": file_uri(root),
                "name": root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            }],
            "capabilities": capabilities(),
        });
        if !spec.init.is_null() {
            params["initializationOptions"] = spec.init.clone();
        }
        let answer = match rpc.request("initialize", params, STARTING) {
            Ok(answer) => answer,
            Err(reason) => {
                let words = rpc.last_words();
                rpc.kill();
                return Err(match words {
                    Some(words) => format!("{} did not start: {reason}: {words}", spec.command),
                    None => format!("{} did not start: {reason}", spec.command),
                });
            }
        };
        let capabilities = answer.get("capabilities").cloned().unwrap_or(Value::Null);
        let sync = match capabilities.get("textDocumentSync") {
            Some(Value::Number(kind)) => kind.as_u64().unwrap_or(1),
            Some(options) => options.get("change").and_then(Value::as_u64).unwrap_or(1),
            None => 1,
        };
        let triggers = capabilities
            .get("completionProvider")
            .and_then(|provider| provider.get("triggerCharacters"))
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        rpc.notify("initialized", json!({}))?;
        if !spec.settings.is_null() {
            rpc.notify(
                "workspace/didChangeConfiguration",
                json!({"settings": spec.settings}),
            )?;
        }
        Ok(Self {
            rpc,
            abilities: capabilities,
            raw,
            applied,
            sync,
            triggers,
            sink,
            pulled: Mutex::new(HashMap::new()),
        })
    }

    /// Whether the server is still running.
    pub(crate) fn is_alive(&self) -> bool {
        self.rpc.is_alive()
    }

    /// The characters after which the server offers completions without being asked.
    pub(crate) fn triggers(&self) -> &[String] {
        &self.triggers
    }

    /// Tells the server the file at `uri`, written in `language`, is open, holding `text` at `version`.
    pub(crate) fn open(
        &self,
        uri: &str,
        language: &str,
        version: i32,
        text: &str,
    ) -> Result<(), String> {
        self.rpc.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": language, "version": version, "text": text}}),
        )
    }

    /// Tells the server the file at `uri` was changed by `changes` and now holds `text` at `version`.
    pub(crate) fn change(
        &self,
        uri: &str,
        version: i32,
        changes: &[TextChange],
        text: &str,
    ) -> Result<(), String> {
        let content: Vec<Value> = match self.sync {
            0 => return Ok(()),
            2 => changes
                .iter()
                .map(|change| match change.span {
                    Some(span) => json!({"range": range_value(span), "text": change.text}),
                    None => json!({"text": change.text}),
                })
                .collect(),
            _ => vec![json!({"text": text})],
        };
        self.rpc.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": content}),
        )
    }

    /// Tells the server the file at `uri` was closed.
    pub(crate) fn close(&self, uri: &str) -> Result<(), String> {
        self.rpc.notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": uri}}),
        )
    }

    /// What could be written at `position` of the file at `uri`, typed after `trigger` when a trigger character asked for them.
    pub(crate) fn completion(
        &self,
        uri: &str,
        position: Position,
        trigger: Option<&str>,
    ) -> Result<Vec<Completion>, String> {
        let context = match trigger {
            Some(character) => json!({"triggerKind": 2, "triggerCharacter": character}),
            None => json!({"triggerKind": 1}),
        };
        let answer = self.rpc.request(
            "textDocument/completion",
            json!({"textDocument": {"uri": uri}, "position": position_value(position), "context": context}),
            ANSWERING,
        )?;
        let default_span = answer
            .get("itemDefaults")
            .and_then(|defaults| defaults.get("editRange"))
            .and_then(|range| span_of(range).or_else(|| range.get("insert").and_then(span_of)));
        let items = match &answer {
            Value::Array(items) => items.clone(),
            other => other
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        };
        Ok(items
            .iter()
            .filter_map(|item| completion_of(item, default_span))
            .collect())
    }

    /// What the server says about the thing at `position` of the file at `uri`.
    pub(crate) fn hover(&self, uri: &str, position: Position) -> Result<Option<Hover>, String> {
        let answer = self.rpc.request(
            "textDocument/hover",
            json!({"textDocument": {"uri": uri}, "position": position_value(position)}),
            ANSWERING,
        )?;
        if answer.is_null() {
            return Ok(None);
        }
        let text = markup_of(answer.get("contents").unwrap_or(&Value::Null));
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(Hover {
            text,
            span: answer.get("range").and_then(span_of),
        }))
    }

    /// Whether the server answers `provider`, one of the abilities it named when it started.
    fn can(&self, provider: &str) -> bool {
        self.abilities
            .get(provider)
            .is_some_and(|ability| !ability.is_null() && ability.as_bool() != Some(false))
    }

    /// Whether the server formats a stretch of a file.
    pub(crate) fn formats_ranges(&self) -> bool {
        self.can("documentRangeFormattingProvider")
    }

    /// Whether the server says which blocks of a file can be folded.
    pub(crate) fn folds(&self) -> bool {
        self.can("foldingRangeProvider")
    }

    /// Whether the server says which stretches of a file are edited together.
    pub(crate) fn links(&self) -> bool {
        self.can("linkedEditingRangeProvider")
    }

    /// Whether the server says what each name of a file is.
    pub(crate) fn has_semantic_tokens(&self) -> bool {
        self.can("semanticTokensProvider")
            && self
                .abilities
                .get("semanticTokensProvider")
                .is_some_and(|provider| match provider.get("full") {
                    Some(full) => !full.is_null() && full.as_bool() != Some(false),
                    None => false,
                })
    }

    /// Whether the server says what to show inside a line.
    pub(crate) fn has_inlay_hints(&self) -> bool {
        self.can("inlayHintProvider")
    }

    /// Whether the server takes part in renames of files: `operation` is `willRename` to be asked for edits before one, `didRename` to be told after.
    fn takes_part_in(&self, operation: &str) -> bool {
        self.abilities
            .get("workspace")
            .and_then(|workspace| workspace.get("fileOperations"))
            .and_then(|operations| operations.get(operation))
            .is_some_and(|options| !options.is_null())
    }

    /// The edit across the project that the server asks for before the file or folder at `from` is renamed to `to`, as it writes it; nothing when it takes no part in renames.
    pub(crate) fn will_rename(&self, from: &str, to: &str) -> Result<Value, String> {
        if !self.takes_part_in("willRename") {
            return Ok(Value::Null);
        }
        self.rpc.request(
            "workspace/willRenameFiles",
            json!({"files": [{"oldUri": from, "newUri": to}]}),
            ANSWERING,
        )
    }

    /// Tells the server, when it takes part in renames, that the file or folder at `from` was renamed to `to`.
    pub(crate) fn did_rename(&self, from: &str, to: &str) -> Result<(), String> {
        if !self.takes_part_in("didRename") {
            return Ok(());
        }
        self.rpc.notify(
            "workspace/didRenameFiles",
            json!({"files": [{"oldUri": from, "newUri": to}]}),
        )
    }

    /// Whether the server gives the diagnostics of a file when asked, instead of publishing them.
    pub(crate) fn pulls_diagnostics(&self) -> bool {
        self.can("diagnosticProvider")
    }

    /// Asks the server for the diagnostics of the file at `uri`, which holds `version`, once changes have stopped coming for a moment, handing the answer to the sink; a newer ask of the same file before then takes its place.
    pub(crate) fn pull_diagnostics_soon(server: &Arc<Self>, uri: &str, version: i32) {
        let turn = {
            let Ok(mut pulled) = server.pulled.lock() else {
                return;
            };
            let held = pulled.entry(uri.to_owned()).or_default();
            held.0 += 1;
            held.0
        };
        let (server, uri) = (server.clone(), uri.to_owned());
        std::thread::spawn(move || {
            std::thread::sleep(PULL_AFTER);
            let latest = server
                .pulled
                .lock()
                .is_ok_and(|pulled| pulled.get(&uri).is_some_and(|held| held.0 == turn));
            if latest {
                let _ = server.pull_diagnostics(&uri, version);
            }
        });
    }

    /// Asks the server for the diagnostics of the file at `uri`, which holds `version`, and hands them to the sink; an answer that nothing changed gives again what it said before.
    fn pull_diagnostics(&self, uri: &str, version: i32) -> Result<(), String> {
        let before = self
            .pulled
            .lock()
            .ok()
            .and_then(|pulled| pulled.get(uri).and_then(|held| held.1.clone()));
        let mut params = json!({"textDocument": {"uri": uri}});
        if let Some(before) = before {
            params["previousResultId"] = json!(before);
        }
        let answer = self
            .rpc
            .request("textDocument/diagnostic", params, ANSWERING)?;
        if let Ok(mut pulled) = self.pulled.lock() {
            pulled.entry(uri.to_owned()).or_default().1 = answer
                .get("resultId")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        let list = match answer.get("items").and_then(Value::as_array) {
            Some(items) => {
                if let Ok(mut held) = self.raw.lock() {
                    held.insert(uri.to_owned(), items.clone());
                }
                items.clone()
            }
            None => self
                .raw
                .lock()
                .ok()
                .and_then(|held| held.get(uri).cloned())
                .unwrap_or_default(),
        };
        (self.sink)(
            uri.to_owned(),
            Some(version),
            list.iter().filter_map(diagnostic_of).collect(),
        );
        Ok(())
    }

    /// Whether the server says which names of a file to show the values of while a program is paused.
    pub(crate) fn has_inline_values(&self) -> bool {
        self.can("inlineValueProvider")
    }

    /// Which stretches of the file at `uri` within `range` to show the values of while the program is paused at `stopped`, in the frame the debugger calls `frame`.
    pub(crate) fn inline_values(
        &self,
        uri: &str,
        range: Span,
        frame: i64,
        stopped: Span,
    ) -> Result<Vec<InlineValue>, String> {
        if !self.has_inline_values() {
            return Ok(Vec::new());
        }
        let answer = self.rpc.request(
            "textDocument/inlineValue",
            json!({
                "textDocument": {"uri": uri},
                "range": range_value(range),
                "context": {"frameId": frame, "stoppedLocation": range_value(stopped)},
            }),
            ANSWERING,
        )?;
        Ok(answer
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(inline_value_of)
            .collect())
    }

    /// Whether the server says what to show above a line.
    pub(crate) fn has_code_lens(&self) -> bool {
        self.can("codeLensProvider")
    }

    /// The characters after which the server formats what was just typed.
    pub(crate) fn format_triggers(&self) -> Vec<String> {
        let provider = self.abilities.get("documentOnTypeFormattingProvider");
        let first = provider
            .and_then(|provider| provider.get("firstTriggerCharacter"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let more = provider
            .and_then(|provider| provider.get("moreTriggerCharacter"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_owned));
        first.into_iter().chain(more).collect()
    }

    /// The options a server formats with: `indent` columns a level, tabs when `tabs`.
    fn format_options(indent: usize, tabs: bool) -> Value {
        json!({"tabSize": indent, "insertSpaces": !tabs, "trimTrailingWhitespace": true, "insertFinalNewline": true})
    }

    /// The edits a server answered a formatting request with, each a stretch of the text as it is now and what replaces it.
    fn edits_of(answer: &Value) -> Vec<TextChange> {
        answer
            .as_array()
            .map(|edits| {
                edits
                    .iter()
                    .filter_map(|edit| {
                        Some(TextChange {
                            span: Some(span_of(edit.get("range")?)?),
                            text: edit.get("newText")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The edits that format the file at `uri`, or only the stretch `range` of it, indented by `indent` columns a level, with tabs when `tabs`, each a stretch of the text as it is now and what replaces it; none when the server cannot format a stretch.
    pub(crate) fn formatting(
        &self,
        uri: &str,
        range: Option<Span>,
        indent: usize,
        tabs: bool,
    ) -> Result<Vec<TextChange>, String> {
        let options = Self::format_options(indent, tabs);
        let answer = match range {
            Some(range) if self.formats_ranges() => self.rpc.request(
                "textDocument/rangeFormatting",
                json!({"textDocument": {"uri": uri}, "range": range_value(range), "options": options}),
                ANSWERING,
            )?,
            Some(_) => return Ok(Vec::new()),
            None => self.rpc.request(
                "textDocument/formatting",
                json!({"textDocument": {"uri": uri}, "options": options}),
                ANSWERING,
            )?,
        };
        Ok(Self::edits_of(&answer))
    }

    /// The edits that format what was just typed, the character `typed` at `position` of the file at `uri`; none when the server does not format as one types.
    pub(crate) fn formatting_on_type(
        &self,
        uri: &str,
        position: Position,
        typed: &str,
        indent: usize,
        tabs: bool,
    ) -> Result<Vec<TextChange>, String> {
        if !self.can("documentOnTypeFormattingProvider") {
            return Ok(Vec::new());
        }
        let answer = self.rpc.request(
            "textDocument/onTypeFormatting",
            json!({
                "textDocument": {"uri": uri},
                "position": position_value(position),
                "ch": typed,
                "options": Self::format_options(indent, tabs),
            }),
            ANSWERING,
        )?;
        Ok(Self::edits_of(&answer))
    }

    /// The blocks of lines of the file at `uri` that can be folded, as the server gives them.
    pub(crate) fn folding_ranges(&self, uri: &str) -> Result<Vec<FoldRange>, String> {
        if !self.folds() {
            return Ok(Vec::new());
        }
        let answer = self.rpc.request(
            "textDocument/foldingRange",
            json!({"textDocument": {"uri": uri}}),
            ANSWERING,
        )?;
        Ok(answer
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|range| {
                Some(FoldRange {
                    start: range.get("startLine")?.as_u64()? as u32,
                    end: range.get("endLine")?.as_u64()? as u32,
                })
            })
            .filter(|range| range.end > range.start)
            .collect())
    }

    /// The stretches of the file at `uri` that are edited together with the one at `position`, the one at `position` among them, as the server gives them; none when it knows of none.
    pub(crate) fn linked_ranges(&self, uri: &str, position: Position) -> Result<Vec<Span>, String> {
        if !self.links() {
            return Ok(Vec::new());
        }
        let answer = self.rpc.request(
            "textDocument/linkedEditingRange",
            json!({"textDocument": {"uri": uri}, "position": position_value(position)}),
            ANSWERING,
        )?;
        Ok(answer
            .get("ranges")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(span_of)
            .collect())
    }

    /// The places `seek` looks for from `position` of the file at `uri`, as the addresses and stretches the server gives, with the names of the functions for calls.
    pub(crate) fn locate(
        &self,
        uri: &str,
        position: Position,
        seek: Seek,
    ) -> Result<Vec<Found>, String> {
        let at = json!({"textDocument": {"uri": uri}, "position": position_value(position)});
        let method = match seek {
            Seek::Definition => "textDocument/definition",
            Seek::Declaration => "textDocument/declaration",
            Seek::TypeDefinition => "textDocument/typeDefinition",
            Seek::Implementation => "textDocument/implementation",
            Seek::References => {
                let mut params = at;
                params["context"] = json!({"includeDeclaration": true});
                let answer = self
                    .rpc
                    .request("textDocument/references", params, ANSWERING)?;
                return Ok(places_of(answer));
            }
            Seek::IncomingCalls | Seek::OutgoingCalls | Seek::Supertypes | Seek::Subtypes => {
                return self.hierarchy(at, seek);
            }
        };
        Ok(places_of(self.rpc.request(method, at, ANSWERING)?))
    }

    /// The functions or types related to the one at the place `at` as `seek` says: the functions that call it or are called by it, the types it derives from or that derive from it.
    fn hierarchy(&self, at: Value, seek: Seek) -> Result<Vec<Found>, String> {
        let (prepare, method, side) = match seek {
            Seek::IncomingCalls => (
                "textDocument/prepareCallHierarchy",
                "callHierarchy/incomingCalls",
                Some("from"),
            ),
            Seek::OutgoingCalls => (
                "textDocument/prepareCallHierarchy",
                "callHierarchy/outgoingCalls",
                Some("to"),
            ),
            Seek::Supertypes => (
                "textDocument/prepareTypeHierarchy",
                "typeHierarchy/supertypes",
                None,
            ),
            _ => (
                "textDocument/prepareTypeHierarchy",
                "typeHierarchy/subtypes",
                None,
            ),
        };
        let prepared = self.rpc.request(prepare, at, ANSWERING)?;
        let mut found = Vec::new();
        for item in prepared.as_array().into_iter().flatten() {
            let related = self.rpc.request(method, json!({"item": item}), ANSWERING)?;
            for entry in related.as_array().into_iter().flatten() {
                let other = match side {
                    Some(side) => entry.get(side),
                    None => Some(entry),
                };
                let Some(other) = other else {
                    continue;
                };
                let (Some(address), Some(span)) = (
                    other.get("uri").and_then(Value::as_str),
                    other
                        .get("selectionRange")
                        .or_else(|| other.get("range"))
                        .and_then(span_of),
                ) else {
                    continue;
                };
                let name = other.get("name").and_then(Value::as_str).map(str::to_owned);
                found.push((address.to_owned(), span, name));
            }
        }
        Ok(found)
    }

    /// The symbols of the project whose names match `query`, as the server finds them, each with the place it is defined and its name.
    pub(crate) fn workspace_symbols(&self, query: &str) -> Result<Vec<Found>, String> {
        let answer = self
            .rpc
            .request("workspace/symbol", json!({"query": query}), ANSWERING)?;
        Ok(answer
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|symbol| {
                let name = symbol.get("name")?.as_str()?.to_owned();
                let location = symbol.get("location")?;
                let address = location.get("uri")?.as_str()?.to_owned();
                let span = location.get("range").and_then(span_of).unwrap_or_default();
                let container = symbol
                    .get("containerName")
                    .and_then(Value::as_str)
                    .filter(|container| !container.is_empty());
                let shown = match container {
                    Some(container) => format!("{name}  {container}"),
                    None => name,
                };
                Some((address, span, Some(shown)))
            })
            .collect())
    }

    /// The stretches of the file at `uri` where the thing at `position` is used, as the server gives them.
    pub(crate) fn highlights(&self, uri: &str, position: Position) -> Result<Vec<Span>, String> {
        let answer = self.rpc.request(
            "textDocument/documentHighlight",
            json!({"textDocument": {"uri": uri}, "position": position_value(position)}),
            ANSWERING,
        )?;
        Ok(answer
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| span_of(item.get("range")?))
            .collect())
    }

    /// What each name of the file at `uri` is, as the server says, one token to a stretch of a line.
    pub(crate) fn semantic_tokens(&self, uri: &str) -> Result<Vec<SemanticToken>, String> {
        if !self.has_semantic_tokens() {
            return Ok(Vec::new());
        }
        let legend = self
            .abilities
            .get("semanticTokensProvider")
            .and_then(|provider| provider.get("legend"));
        let names = |key: &str| -> Vec<String> {
            legend
                .and_then(|legend| legend.get(key))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        };
        let (types, modifiers) = (names("tokenTypes"), names("tokenModifiers"));
        let answer = self.rpc.request(
            "textDocument/semanticTokens/full",
            json!({"textDocument": {"uri": uri}}),
            ANSWERING,
        )?;
        let data: Vec<u64> = answer
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .collect();
        let (mut line, mut start) = (0u32, 0u32);
        let mut tokens = Vec::with_capacity(data.len() / 5);
        for group in data.as_chunks::<5>().0 {
            let (delta_line, delta_start) = (group[0] as u32, group[1] as u32);
            line += delta_line;
            start = if delta_line == 0 {
                start + delta_start
            } else {
                delta_start
            };
            let name = types.get(group[3] as usize).map_or("", String::as_str);
            let has = |wanted: &str| {
                modifiers
                    .iter()
                    .position(|held| held == wanted)
                    .is_some_and(|bit| group[4] >> bit & 1 == 1)
            };
            tokens.push(SemanticToken {
                line,
                start,
                length: group[2] as u32,
                kind: token_kind(name, has("readonly") || has("static")),
            });
        }
        Ok(tokens)
    }

    /// What to show inside the lines of the file at `uri` within `span`, as the server says.
    pub(crate) fn inlay_hints(&self, uri: &str, span: Span) -> Result<Vec<InlayHint>, String> {
        if !self.has_inlay_hints() {
            return Ok(Vec::new());
        }
        let answer = self.rpc.request(
            "textDocument/inlayHint",
            json!({"textDocument": {"uri": uri}, "range": range_value(span)}),
            ANSWERING,
        )?;
        Ok(answer
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(hint_of)
            .take(MOST_HINTS)
            .collect())
    }

    /// The lines of text the server says to show with stretches of the file at `uri`, and each as the server wrote it, to be pressed by its place in the list; a lens the server has yet to fill in is asked to.
    pub(crate) fn code_lenses(&self, uri: &str) -> Result<(Vec<CodeLens>, Vec<Value>), String> {
        if !self.has_code_lens() {
            return Ok((Vec::new(), Vec::new()));
        }
        let answer = self.rpc.request(
            "textDocument/codeLens",
            json!({"textDocument": {"uri": uri}}),
            ANSWERING,
        )?;
        let resolves = self
            .abilities
            .get("codeLensProvider")
            .and_then(|provider| provider.get("resolveProvider"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut lenses = Vec::new();
        let mut raw = Vec::new();
        for lens in answer.as_array().into_iter().flatten().take(MOST_LENSES) {
            let lens = if lens.get("command").is_none() && resolves {
                self.rpc
                    .request("codeLens/resolve", lens.clone(), ANSWERING)?
            } else {
                lens.clone()
            };
            let (Some(span), Some(title)) = (
                lens.get("range").and_then(span_of),
                lens.get("command")
                    .and_then(|command| command.get("title"))
                    .and_then(Value::as_str)
                    .filter(|title| !title.is_empty()),
            ) else {
                continue;
            };
            lenses.push(CodeLens {
                span,
                title: title.to_owned(),
                pressable: self.presses(&lens),
                runs: false,
            });
            raw.push(lens);
        }
        Ok((lenses, raw))
    }

    /// Whether pressing `lens`, as the server wrote it, does something: it shows places, or the server carries out its command.
    fn presses(&self, lens: &Value) -> bool {
        let Some(command) = lens
            .get("command")
            .and_then(|command| command.get("command"))
            .and_then(Value::as_str)
        else {
            return false;
        };
        shows_places(command)
            || self
                .abilities
                .get("executeCommandProvider")
                .and_then(|provider| provider.get("commands"))
                .and_then(Value::as_array)
                .is_some_and(|commands| commands.iter().any(|held| held.as_str() == Some(command)))
    }

    /// Carries out the command of `lens`, as the server wrote it: the places it points to, or the edits the server makes.
    pub(crate) fn press_lens(&self, lens: &Value) -> Result<Pressed, String> {
        let command = lens
            .get("command")
            .ok_or_else(|| "that lens says nothing to do".to_owned())?;
        let name = command
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let arguments = command.get("arguments").cloned().unwrap_or(json!([]));
        if shows_places(name) {
            let places = arguments
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|argument| argument.as_array().filter(|list| !list.is_empty()))
                .cloned()
                .unwrap_or_default();
            return Ok(Pressed::Places(places_of(Value::Array(places))));
        }
        if let Ok(mut held) = self.applied.lock() {
            held.clear();
        }
        self.rpc.request(
            "workspace/executeCommand",
            json!({"command": name, "arguments": arguments}),
            ANSWERING,
        )?;
        let mut edits = Vec::new();
        if let Ok(mut held) = self.applied.lock() {
            edits.append(&mut held);
        }
        Ok(Pressed::Edits(edits))
    }

    /// The characters after which the server tells what the call the caret is in looks like.
    pub(crate) fn signature_triggers(&self) -> Vec<String> {
        let provider = self.abilities.get("signatureHelpProvider");
        ["triggerCharacters", "retriggerCharacters"]
            .iter()
            .flat_map(|key| {
                provider
                    .and_then(|provider| provider.get(*key))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect()
    }

    /// The signature of the call at `position` of the file at `uri`, with the parameter it is at.
    pub(crate) fn signature(
        &self,
        uri: &str,
        position: Position,
    ) -> Result<Option<Signature>, String> {
        let answer = self.rpc.request(
            "textDocument/signatureHelp",
            json!({"textDocument": {"uri": uri}, "position": position_value(position), "context": {"triggerKind": 1, "isRetrigger": false}}),
            ANSWERING,
        )?;
        let signatures = answer
            .get("signatures")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let chosen = answer
            .get("activeSignature")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let Some(signature) = signatures.get(chosen).or(signatures.first()) else {
            return Ok(None);
        };
        let label = signature
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let parameter = signature
            .get("activeParameter")
            .or_else(|| answer.get("activeParameter"))
            .and_then(Value::as_u64)
            .map(|index| index as usize);
        let active = parameter
            .and_then(|index| signature.get("parameters")?.as_array()?.get(index).cloned())
            .and_then(|parameter| match parameter.get("label")? {
                Value::String(name) => {
                    let start = label.find(name.as_str())?;
                    Some((start, start + name.len()))
                }
                Value::Array(bounds) => {
                    let start = utf16_to_byte(&label, bounds.first()?.as_u64()? as usize);
                    let end = utf16_to_byte(&label, bounds.get(1)?.as_u64()? as usize);
                    Some((start, end))
                }
                _ => None,
            });
        let documentation = signature
            .get("documentation")
            .map(markup_of)
            .filter(|text| !text.trim().is_empty());
        Ok(Some(Signature {
            label,
            active,
            documentation,
            others: signatures.len().saturating_sub(1),
        }))
    }

    /// The edit across the project that renames the thing at `position` of the file at `uri` to `name`, as the server writes it.
    pub(crate) fn rename(
        &self,
        uri: &str,
        position: Position,
        name: &str,
    ) -> Result<Value, String> {
        self.rpc.request(
            "textDocument/rename",
            json!({"textDocument": {"uri": uri}, "position": position_value(position), "newName": name}),
            ANSWERING,
        )
    }

    /// The changes the server offers for the stretch `span` of the file at `uri`, as it writes them, with what it said is wrong there.
    pub(crate) fn code_actions(&self, uri: &str, span: Span) -> Result<Vec<Value>, String> {
        let wrong: Vec<Value> = self
            .raw
            .lock()
            .ok()
            .and_then(|held| held.get(uri).cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|diagnostic| {
                diagnostic
                    .get("range")
                    .and_then(span_of)
                    .is_some_and(|held| held.start <= span.end && span.start <= held.end)
            })
            .collect();
        let answer = self.rpc.request(
            "textDocument/codeAction",
            json!({
                "textDocument": {"uri": uri},
                "range": range_value(span),
                "context": {"diagnostics": wrong, "triggerKind": 1},
            }),
            ANSWERING,
        )?;
        Ok(answer.as_array().cloned().unwrap_or_default())
    }

    /// Carries out `action`, one of the changes the server offered: the edits it makes, as the server writes them, those of its command among them.
    pub(crate) fn take_action(&self, action: &Value) -> Result<Vec<Value>, String> {
        let resolves = self
            .abilities
            .get("codeActionProvider")
            .and_then(|provider| provider.get("resolveProvider"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_command = action.get("command").is_some_and(Value::is_string);
        let action = if resolves && !is_command && action.get("edit").is_none() {
            self.rpc
                .request("codeAction/resolve", action.clone(), ANSWERING)?
        } else {
            action.clone()
        };
        let mut edits: Vec<Value> = action.get("edit").cloned().into_iter().collect();
        let command = if is_command {
            Some(action.clone())
        } else {
            action.get("command").cloned()
        };
        if let Some(command) = command {
            if let Ok(mut held) = self.applied.lock() {
                held.clear();
            }
            self.rpc.request(
                "workspace/executeCommand",
                json!({"command": command.get("command"), "arguments": command.get("arguments").cloned().unwrap_or(json!([]))}),
                ANSWERING,
            )?;
            if let Ok(mut held) = self.applied.lock() {
                edits.append(&mut held);
            }
        }
        Ok(edits)
    }

    /// Asks the server to stop, and ends it if it does not.
    pub(crate) fn stop(&self) {
        self.rpc.stop(STOPPING);
    }
}

/// What a name a server calls `name` is, as it is coloured, a variable that `fixed` is a constant.
fn token_kind(name: &str, fixed: bool) -> TokenKind {
    match name {
        "namespace" | "type" | "class" | "enum" | "interface" | "struct" | "typeParameter"
        | "builtinType" | "typeAlias" | "trait" | "lifetime" | "union" => TokenKind::Type,
        "function" | "method" | "macro" | "decorator" | "derive" | "attribute" | "event" => {
            TokenKind::Function
        }
        "keyword" | "modifier" | "selfKeyword" | "selfTypeKeyword" | "controlFlow" => {
            TokenKind::Keyword
        }
        "string" | "regexp" | "escapeSequence" | "formatSpecifier" | "character" => {
            TokenKind::String
        }
        "number" | "enumMember" | "boolean" | "constant" | "const" => TokenKind::Constant,
        "comment" => TokenKind::Comment,
        "property" | "field" => TokenKind::Property,
        "operator" | "punctuation" => TokenKind::Operator,
        "variable" | "parameter" if fixed => TokenKind::Constant,
        _ => TokenKind::Variable,
    }
}

/// The inline value a server writes as `value`: text, a variable to look up or an expression to evaluate.
fn inline_value_of(value: &Value) -> Option<InlineValue> {
    let named = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let kind = if let Some(text) = named("text") {
        InlineValueKind::Text(text)
    } else if value.get("caseSensitiveLookup").is_some() {
        InlineValueKind::Variable(named("variableName"))
    } else {
        InlineValueKind::Expression(named("expression"))
    };
    Some(InlineValue {
        span: span_of(value.get("range")?)?,
        kind,
    })
}

/// The inlay hint a server writes as `value`.
fn hint_of(value: &Value) -> Option<InlayHint> {
    let position = value.get("position")?;
    let label = match value.get("label")? {
        Value::String(label) => label.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("value").and_then(Value::as_str))
            .collect(),
        _ => return None,
    };
    if label.trim().is_empty() {
        return None;
    }
    let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
    Some(InlayHint {
        position: Position::new(
            position.get("line")?.as_u64()? as u32,
            position.get("character")?.as_u64()? as u32,
        ),
        label,
        kind: match value.get("kind").and_then(Value::as_u64) {
            Some(1) => HintKind::Type,
            Some(2) => HintKind::Parameter,
            _ => HintKind::Other,
        },
        pad_left: flag("paddingLeft"),
        pad_right: flag("paddingRight"),
    })
}

/// Whether the command a server calls `name` only asks the client to show the places it counted, rather than something the server does.
fn shows_places(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with("showreferences") || name.ends_with("findreferences")
}

/// What a code action the server writes as `value` is to the reader.
pub(crate) fn action_of(value: &Value) -> Option<CodeAction> {
    Some(CodeAction {
        title: value.get("title")?.as_str()?.to_owned(),
        kind: value.get("kind").and_then(Value::as_str).map(str::to_owned),
        preferred: value
            .get("isPreferred")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        disabled: value
            .get("disabled")
            .and_then(|disabled| disabled.get("reason"))
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// The places a server writes as `answer`: one location or link, a list of them, or none.
fn places_of(answer: Value) -> Vec<Found> {
    let items = match answer {
        Value::Array(items) => items,
        Value::Null => Vec::new(),
        single => vec![single],
    };
    items
        .iter()
        .filter_map(|item| {
            let address = item
                .get("targetUri")
                .or_else(|| item.get("uri"))
                .and_then(Value::as_str)?;
            let range = item
                .get("targetSelectionRange")
                .or_else(|| item.get("range"))
                .and_then(span_of)?;
            Some((address.to_owned(), range, None))
        })
        .collect()
}

/// The changes to each file that the edit across a project a server writes as `edit` makes, by address; files it creates, renames or deletes are left out.
pub(crate) fn file_changes(edit: &Value) -> Vec<(String, Vec<TextChange>)> {
    let changes_of = |edits: &Value| -> Vec<TextChange> {
        edits
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|edit| {
                Some(TextChange {
                    span: Some(span_of(edit.get("range")?)?),
                    text: edit.get("newText")?.as_str()?.to_owned(),
                })
            })
            .collect()
    };
    let mut files: Vec<(String, Vec<TextChange>)> = Vec::new();
    let mut add = |address: &str, mut changes: Vec<TextChange>| match files
        .iter_mut()
        .find(|(held, _)| held == address)
    {
        Some((_, held)) => held.append(&mut changes),
        None => files.push((address.to_owned(), changes)),
    };
    if let Some(documents) = edit.get("documentChanges").and_then(Value::as_array) {
        for document in documents {
            if let Some(address) = document
                .get("textDocument")
                .and_then(|held| held.get("uri"))
                .and_then(Value::as_str)
            {
                add(
                    address,
                    changes_of(document.get("edits").unwrap_or(&Value::Null)),
                );
            }
        }
    } else if let Some(changes) = edit.get("changes").and_then(Value::as_object) {
        for (address, edits) in changes {
            add(address, changes_of(edits));
        }
    }
    files
}

/// The byte of `text` that `units` UTF-16 code units into it reach.
fn utf16_to_byte(text: &str, units: usize) -> usize {
    let mut counted = 0;
    for (at, character) in text.char_indices() {
        if counted >= units {
            return at;
        }
        counted += character.len_utf16();
    }
    text.len()
}

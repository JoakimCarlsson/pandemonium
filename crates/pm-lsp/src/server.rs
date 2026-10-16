//! One running language server: started for a project, told of the files open in it, and asked for completions and hovers.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use pm_buffer::{Position, Span};
use serde_json::{Value, json};

use crate::manifest::ServerSpec;
use crate::resolve::{found, managed};
use crate::rpc::{Handler, Rpc};
use crate::snippet;
use crate::types::{Completion, CompletionKind, Diagnostic, Hover, Severity, TextChange};
use crate::uri::file_uri;

/// How long a server may take to start.
const STARTING: Duration = Duration::from_secs(60);

/// How long a server may take to answer a question about a file.
const ANSWERING: Duration = Duration::from_secs(10);

/// How long a server is given to stop before its process is ended.
const STOPPING: Duration = Duration::from_secs(2);

/// What is done with the diagnostics a server publishes: the address of the file, the version they are about and the diagnostics.
pub(crate) type DiagnosticsSink = Arc<dyn Fn(String, Option<i32>, Vec<Diagnostic>) + Send + Sync>;

/// A running language server.
pub(crate) struct LanguageServer {
    /// The conversation with it.
    rpc: Arc<Rpc>,
    /// How it takes changes: 0 not at all, 1 as the whole text, 2 as stretches.
    sync: u64,
    /// The characters after which it offers completions without being asked.
    triggers: Vec<String>,
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
        "workspace": {"configuration": true, "workspaceFolders": true},
        "textDocument": {
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
            "publishDiagnostics": {"versionSupport": true}
        },
        "window": {"workDoneProgress": false}
    })
}

/// What answers the requests of a server started for `root` with `settings`, and hands the diagnostics it publishes to `sink`.
fn handler(root: &Path, settings: Value, sink: DiagnosticsSink) -> Handler {
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
        "textDocument/publishDiagnostics" => {
            if let Some(uri) = params.get("uri").and_then(Value::as_str) {
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
        sink: DiagnosticsSink,
    ) -> Result<Self, String> {
        let first_try = match found(spec, Some(root)) {
            Some(program) => match Self::start_program(&program, spec, root, sink.clone()) {
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
        Self::start_program(&program, spec, root, sink)
    }

    /// Starts `program` as the server of `spec` for the project at `root` and sets it up.
    fn start_program(
        program: &Path,
        spec: &ServerSpec,
        root: &Path,
        sink: DiagnosticsSink,
    ) -> Result<Self, String> {
        let first: Vec<PathBuf> = spec
            .project
            .iter()
            .map(|folder| root.join(folder))
            .collect();
        let rpc = Rpc::start(
            program,
            &spec.args,
            root,
            &first,
            handler(root, spec.settings.clone(), sink),
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
            sync,
            triggers,
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

    /// The edits that format the whole file at `uri`, indented by `indent` columns a level, with tabs when `tabs`, each a stretch of the text as it is now and what replaces it.
    pub(crate) fn formatting(
        &self,
        uri: &str,
        indent: usize,
        tabs: bool,
    ) -> Result<Vec<TextChange>, String> {
        let answer = self.rpc.request(
            "textDocument/formatting",
            json!({
                "textDocument": {"uri": uri},
                "options": {"tabSize": indent, "insertSpaces": !tabs, "trimTrailingWhitespace": true, "insertFinalNewline": true},
            }),
            ANSWERING,
        )?;
        Ok(answer
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
            .unwrap_or_default())
    }

    /// Where the thing at `position` of the file at `uri` is defined, as the addresses and stretches the server gives.
    pub(crate) fn definition(
        &self,
        uri: &str,
        position: Position,
    ) -> Result<Vec<(String, Span)>, String> {
        let answer = self.rpc.request(
            "textDocument/definition",
            json!({"textDocument": {"uri": uri}, "position": position_value(position)}),
            ANSWERING,
        )?;
        let items = match answer {
            Value::Array(items) => items,
            Value::Null => Vec::new(),
            single => vec![single],
        };
        Ok(items
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
                Some((address.to_owned(), range))
            })
            .collect())
    }

    /// Asks the server to stop, and ends it if it does not.
    pub(crate) fn stop(&self) {
        self.rpc.stop(STOPPING);
    }
}

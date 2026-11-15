//! What a language server would say about the sample files, for snapshots
//! of the editor's language features without a server: completions, a hover
//! card, a signature, names coloured by what they are with hints and lenses,
//! the places a name is used, the lightbulb and its changes, renaming,
//! colours, text being dragged and right-to-left text.

use pm_buffer::{Position, Span};
use pm_editor::Document;
use pm_lsp::{
    CodeAction, CodeLens, ColorSwatch, Completion, CompletionKind, HintKind, InlayHint,
    SemanticToken, Signature, TokenKind,
};
use pm_text::Edit;

use crate::open_files::{EditorPopup, OpenFile};

/// Which of the editor's language features a snapshot shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeSample {
    /// The list of completions open after a dot.
    Completions,
    /// The card saying what a name is.
    Hover,
    /// The signature of the call the caret is in.
    Signature,
    /// Names coloured by what they are, hints, lenses, the uses of the name
    /// at the caret and the lightbulb.
    Annotations,
    /// The lightbulb's menu of changes.
    Actions,
    /// The field renaming the name at the caret.
    Rename,
    /// Selected text being dragged to another place.
    Drag,
    /// The colour picker over a colour of a stylesheet.
    Colour,
}

/// A stylesheet whose colours the `Colour` sample shows swatches for.
pub const COLOUR_SAMPLE: &str = ":root {\n  --accent: #3b82f6;\n  --danger: #ef4444;\n  --success: #22c55e;\n}\n\n.button {\n  color: #ffffff;\n  background: var(--accent);\n  border: 1px solid #1d4ed8;\n}\n";

/// Text mixing right-to-left and left-to-right lines and runs.
pub const RIGHT_TO_LEFT_SAMPLE: &str = "# Greetings in several scripts\n\ngreeting.en = \"Hello, world\"\ngreeting.he = \"שלום עולם\"\ngreeting.ar = \"مرحبا بالعالم\"\nmixed = \"Pandemonium פנדמוניום 2026\"\nلغة = \"العربية\"\n";

/// The span of `range` of `document`.
fn span_of(document: &Document, range: std::ops::Range<usize>) -> Span {
    Span::new(
        document.position_of(range.start),
        document.position_of(range.end),
    )
}

/// The byte where `needle` is first found in `document` at or after `from`.
fn find(document: &Document, needle: &str, from: usize) -> Option<usize> {
    document.field.text()[from..]
        .find(needle)
        .map(|place| place + from)
}

/// Shows on `file`, whose document holds the sample code or stylesheet,
/// what its language server would say for `sample`.
pub fn sample_code(file: &mut OpenFile, sample: CodeSample) {
    let shared = file.document.clone();
    let mut document = shared.0.borrow_mut();
    document.synced = true;
    document.version = 1;
    let root = find(&document, "fn root", 0).unwrap_or(0);
    let version = document.version;
    match sample {
        CodeSample::Colour => {
            let colours: Vec<ColorSwatch> = ["#3b82f6", "#ef4444", "#22c55e", "#ffffff", "#1d4ed8"]
                .iter()
                .filter_map(|hex| {
                    let at = find(&document, hex, 0)?;
                    let value = u32::from_str_radix(&hex[1..], 16).ok()?;
                    Some(ColorSwatch {
                        span: span_of(&document, at..at + hex.len()),
                        red: (value >> 16) as u8,
                        green: (value >> 8) as u8,
                        blue: value as u8,
                        alpha: 0xff,
                    })
                })
                .collect();
            document.take_colors(version, colours);
            drop(document);
            file.popup = Some(EditorPopup::Colour(0));
        }
        CodeSample::Completions => {
            let dot =
                find(&document, "revision.update", root).map_or(root, |at| at + "revision.".len());
            document.edit(Edit::Place(dot, false));
            document.open_completion((0.0, 0.0));
            let item = |label: &str, detail: &str, kind: CompletionKind| Completion {
                label: label.to_owned(),
                detail: Some(detail.to_owned()),
                kind,
                text: label.to_owned(),
                filter: label.to_owned(),
                sort: label.to_owned(),
                ..Completion::default()
            };
            document.take_completions(
                version,
                vec![
                    item(
                        "update",
                        "fn(&self, f: impl FnOnce(&mut T))",
                        CompletionKind::Function,
                    ),
                    item("get", "fn(&self) -> T", CompletionKind::Function),
                    item("set", "fn(&self, value: T)", CompletionKind::Function),
                    item(
                        "with",
                        "fn(&self, f: impl FnOnce(&T) -> R) -> R",
                        CompletionKind::Function,
                    ),
                    item("clone", "fn(&self) -> State<T>", CompletionKind::Function),
                ],
            );
            if let Some(completion) = &mut document.completion {
                completion.items[0].documentation =
                    Some("Changes the value in place and redraws what reads it.".to_owned());
                completion.shown = (0..completion.items.len()).collect();
            }
        }
        CodeSample::Hover => {
            let at = find(&document, "remember(", root).unwrap_or(root);
            document.edit(Edit::Place(at, false));
            document.hover = Some(pm_editor::HoverCard {
                offset: at,
                at: (0.0, 0.0),
                text: Some(
                    "```rust\npub fn remember<T: Clone + 'static>(init: impl FnOnce() -> T) -> State<T>\n```\n\nKeeps a value across frames, made by `init` the first time it is asked for."
                        .to_owned(),
                ),
                diagnostics: Vec::new(),
                scroll: 0.0,
            });
        }
        CodeSample::Signature => {
            let at = find(&document, "remember(|| 0_u64)", root)
                .map_or(root, |at| at + "remember(".len());
            document.edit(Edit::Place(at, false));
            let label = "remember<T>(init: impl FnOnce() -> T) -> State<T>";
            let start = label.find("init").unwrap_or(0);
            let end = label.find(") ->").unwrap_or(label.len());
            document.take_signature(
                version,
                Some(Signature {
                    label: label.to_owned(),
                    active: Some((start, end)),
                    documentation: Some("Keeps a value across frames.".to_owned()),
                    others: 0,
                }),
            );
        }
        CodeSample::Annotations | CodeSample::Actions | CodeSample::Rename | CodeSample::Drag => {
            let name = find(&document, "revision", root).unwrap_or(root);
            annotate(&mut document, root, name);
            drop(document);
            file.actions = vec![
                action(
                    "Rename `revision` to `generation`",
                    "refactor.rewrite",
                    true,
                ),
                action("Extract into a function", "refactor.extract", false),
                action("Inline variable", "refactor.inline", false),
                action("Add `#[must_use]`", "quickfix", false),
            ];
            match sample {
                CodeSample::Actions => file.popup = Some(EditorPopup::Actions),
                CodeSample::Rename => {
                    file.rename = Some((name..name + "revision".len(), "revision".to_owned()))
                }
                CodeSample::Drag => {
                    let mut document = shared.0.borrow_mut();
                    let start = find(&document, "let installed", root).unwrap_or(root);
                    let end = start + "let installed = remember(|| false);".len();
                    document.edit(Edit::Select(start, end));
                    document.drag_started(start + 4);
                    let line = document.line_of(start);
                    let target = document.line(line + 5).start + 8;
                    document.drag_over(target);
                }
                _ => {}
            }
        }
    }
}

/// A change offered at the caret, titled `title`, of `kind`, the one to
/// take when `preferred`.
fn action(title: &str, kind: &str, preferred: bool) -> CodeAction {
    CodeAction {
        title: title.to_owned(),
        kind: Some(kind.to_owned()),
        preferred,
        disabled: None,
    }
}

/// Puts on `document` what a server says about the function starting at
/// `root`, with the caret on the name at `name`: names coloured, hints
/// inside its lines, a lens above it, the uses of the name and the lightbulb.
fn annotate(document: &mut Document, root: usize, name: usize) {
    let version = document.version;
    document.edit(Edit::Place(name + 2, false));
    let end = find(document, "\n}\n", root).unwrap_or(document.field.text().len());
    let body = document.field.text()[root..end].to_owned();
    let mut tokens = Vec::new();
    let mut uses = Vec::new();
    for (word, kind) in [
        ("link", TokenKind::Variable),
        ("revision", TokenKind::Variable),
        ("installed", TokenKind::Variable),
        ("Rc", TokenKind::Type),
        ("RefCell", TokenKind::Type),
        ("Link", TokenKind::Type),
        ("remember", TokenKind::Function),
    ] {
        for (place, _) in body.match_indices(word) {
            let at = root + place;
            let position = document.position_of(at);
            tokens.push(SemanticToken {
                line: position.line,
                start: position.character,
                length: word.len() as u32,
                kind,
            });
            if word == "revision" {
                uses.push(span_of(document, at..at + word.len()));
            }
        }
    }
    document.take_semantic_tokens(version, tokens);
    document.take_highlights(version, uses);
    let hints: Vec<InlayHint> = [
        ("let revision", ": State<u64>"),
        ("let installed", ": State<bool>"),
        ("let settings_open", ": State<bool>"),
    ]
    .iter()
    .filter_map(|(before, label)| {
        let at = find(document, before, root)? + before.len();
        Some(InlayHint {
            position: document.position_of(at),
            label: (*label).to_owned(),
            kind: HintKind::Type,
            pad_left: false,
            pad_right: false,
        })
    })
    .collect();
    document.take_inlay_hints(version, hints);
    let start = document.position_of(root);
    let lens_span = Span::new(start, Position::new(start.line, start.character + 7));
    document.take_code_lenses(
        version,
        vec![
            CodeLens {
                span: lens_span,
                title: "2 references".to_owned(),
                pressable: true,
            },
            CodeLens {
                span: lens_span,
                title: "Run".to_owned(),
                pressable: true,
            },
        ],
    );
    let line = document.line_of(document.field.caret());
    document.take_bulb(version, line, 4);
}

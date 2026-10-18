//! The values of names shown at the end of the lines of the file a program being debugged is paused in.

use pm_editor::Document;
use pm_protocol::{DebugView, InlineValue, InlineValueKind, Position, Span, Standing};

use crate::state::{Outgoing, WorkspaceState};

/// The longest a label of values at the end of a line grows, in characters.
const LONGEST_LABEL: usize = 80;

/// The longest one value is written, in characters.
const LONGEST_VALUE: usize = 40;

/// What the language server was asked for the file a program is paused in, and what it said.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineAsked {
    /// The session, the file, its version, the frame looked at and the line it is paused on.
    pub key: (usize, String, i32, i64, usize),
    /// What the server said to show, once it has.
    pub values: Vec<InlineValue>,
}

/// `text` cut to `most` characters, with an ellipsis where it was cut.
fn shortened(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let kept: String = text.chars().take(most.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// What the variable `name` holds in the frame `view` is looking at, searching the variables of its scopes in order.
fn value_of_variable(view: &DebugView, name: &str) -> Option<String> {
    view.scopes
        .iter()
        .filter_map(|scope| view.variables_of(scope.reference))
        .flatten()
        .find(|variable| variable.name == name)
        .map(|variable| variable.value.clone())
}

/// The label each row gets for `values` of `document`, resolved against `view`: the names the stretches spell with what they hold, one to a name, joined.
fn labels(document: &Document, view: &DebugView, values: &[InlineValue]) -> Vec<(usize, String)> {
    let mut rows: Vec<(usize, Vec<String>, Vec<String>)> = Vec::new();
    for value in values {
        let spelled = || document.text_of(value.span).to_owned();
        let (name, shown) = match &value.kind {
            InlineValueKind::Text(text) => (text.clone(), text.clone()),
            InlineValueKind::Variable(name) | InlineValueKind::Expression(name) => {
                let name = name.clone().unwrap_or_else(spelled);
                let Some(held) = value_of_variable(view, &name) else {
                    continue;
                };
                let shown = format!("{name} = {}", shortened(&held, LONGEST_VALUE));
                (name, shown)
            }
        };
        let row = document.row_of(value.span.start);
        match rows.iter_mut().find(|(held, _, _)| *held == row) {
            Some((_, names, shows)) if !names.contains(&name) => {
                names.push(name);
                shows.push(shown);
            }
            Some(_) => {}
            None => rows.push((row, vec![name], vec![shown])),
        }
    }
    rows.sort_by_key(|(row, _, _)| *row);
    rows.into_iter()
        .map(|(row, _, shows)| (row, shortened(&shows.join(", "), LONGEST_LABEL)))
        .collect()
}

impl WorkspaceState {
    /// Where the program being debugged in the session at `session` is paused, as the frame being looked at, its line and its column, when that is in the file at `path`.
    fn debug_stop_in(&self, session: usize, path: &str) -> Option<(i64, usize, usize)> {
        let run = self
            .debug
            .live()
            .filter(|run| run.session == session && run.view.standing == Standing::Stopped)?;
        let frame = run
            .view
            .selected_frame()
            .or_else(|| run.view.frames.first())?;
        let relative = self.debug_relative(session, frame.path.as_deref()?)?;
        (relative == path).then_some((frame.id, frame.line, frame.column))
    }

    /// Asks the language server which names of the file shown to show the values of when the program is paused in it and the server has not been asked for this pause, shows what it said, and takes the values away when the program is not paused there.
    pub(crate) fn inline_values_requested(mut self) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self
            .shown_viewer()
            .filter(|viewer| !viewer.head)
            .map(|viewer| viewer.path.clone())
        else {
            return self;
        };
        let stop = self.debug_stop_in(session, &path);
        let Some((version, span)) = self
            .shown_text()
            .filter(|document| document.inline_values_supported && document.synced)
            .map(|document| (document.version, document.whole_span()))
        else {
            return self;
        };
        let Some((frame, line, column)) = stop else {
            self.debug.inline = None;
            if let Some(document) = self.shown_text() {
                document.show_values(Vec::new());
            }
            return self;
        };
        let key = (session, path.clone(), version, frame, line);
        if self
            .debug
            .inline
            .as_ref()
            .is_none_or(|held| held.key != key)
        {
            self.debug.inline = Some(InlineAsked {
                key,
                values: Vec::new(),
            });
            let place = Position {
                line: line as u32,
                character: column as u32,
            };
            self.outbox.push(Outgoing::InlineValues {
                session,
                path,
                version,
                span,
                frame,
                stopped: Span {
                    start: place,
                    end: place,
                },
            });
        }
        let (inline, run) = (self.debug.inline.take(), self.debug.run.take());
        if let (Some(inline), Some(run), Some(document)) = (&inline, &run, self.shown_text()) {
            document.show_values(labels(document, &run.view, &inline.values));
        }
        self.debug.inline = inline;
        self.debug.run = run;
        self
    }

    /// The state after the language server said which names of the file at `path` of the session at `session`, as the text was at `version`, to show the values of.
    pub fn with_inline_values(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        values: Vec<InlineValue>,
    ) -> Self {
        if let Some(asked) =
            self.debug.inline.as_mut().filter(|asked| {
                asked.key.0 == session && asked.key.1 == path && asked.key.2 == version
            })
        {
            asked.values = values;
        }
        self
    }
}

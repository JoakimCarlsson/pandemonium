//! The Search tab's editable excerpts on the link's side: reads the files
//! holding lines found to cut stretches around them, and writes the edited
//! stretches back into their files, each read again first so that a
//! stretch changed meanwhile is refused rather than overwritten.

use pm_editor::Document;
use pm_protocol::{Request, Response};
use pm_treesitter::{Syntax, registry};

use super::{Ask, Link};
use crate::open_files::SharedDocument;
use crate::search::{SearchAction, SearchExcerpt, excerpt_ranges, splice_excerpts};

/// What a request the Search tab's excerpts made was for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExcerptAsk {
    /// The text of the file at the path, to cut excerpts from.
    Read(String),
    /// The text of the file at the path, to write its edited excerpts into.
    Save(String),
    /// Writing the file at the path.
    Written(String),
}

impl Link {
    /// Shows the lines found as excerpts that can be edited, reading their files, or as plain lines again.
    pub(super) fn search_edit(&mut self) {
        let search = &mut self.workspace.search;
        search.editing = !search.editing;
        search.excerpts.clear();
        if search.editing {
            self.read_excerpts();
        }
    }

    /// Asks for the text of every file holding lines found, to cut excerpts from.
    pub(super) fn read_excerpts(&mut self) {
        let Some(directory) = self.browse_root() else {
            return;
        };
        let paths: Vec<String> = self
            .workspace
            .search
            .grouped()
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        for path in paths {
            self.ask(
                Request::ReadFile {
                    directory: directory.clone(),
                    path: path.clone(),
                    head: false,
                    revision: None,
                },
                Ask::Excerpt(ExcerptAsk::Read(path)),
            );
        }
    }

    /// Writes the edited excerpts of the file at `path` into it, reading it again first.
    pub(super) fn save_excerpts(&mut self, path: String) {
        let Some(directory) = self.browse_root() else {
            return;
        };
        self.ask(
            Request::ReadFile {
                directory,
                path: path.clone(),
                head: false,
                revision: None,
            },
            Ask::Excerpt(ExcerptAsk::Save(path)),
        );
    }

    /// Handles the `response` to a request the excerpts made for `ask`.
    pub(super) fn answer_excerpt(&mut self, ask: ExcerptAsk, response: Response) {
        match (ask, response) {
            (
                ExcerptAsk::Read(path),
                Response::File {
                    text: Some(text),
                    language,
                    ..
                },
            ) => self.cut_excerpts(&path, &text, language.as_deref()),
            (
                ExcerptAsk::Save(path),
                Response::File {
                    text: Some(text), ..
                },
            ) => {
                let edits: Vec<(usize, String, String)> = self
                    .workspace
                    .search
                    .excerpts
                    .iter()
                    .filter(|excerpt| excerpt.path == path)
                    .map(|excerpt| {
                        (
                            excerpt.first,
                            excerpt.original.clone(),
                            excerpt.document.0.borrow().field.text().to_owned(),
                        )
                    })
                    .collect();
                match splice_excerpts(&text, &edits) {
                    Ok(written) => {
                        let Some(directory) = self.browse_root() else {
                            return;
                        };
                        self.ask(
                            Request::WriteFile {
                                directory,
                                path: path.clone(),
                                text: written,
                            },
                            Ask::Excerpt(ExcerptAsk::Written(path)),
                        );
                    }
                    Err(line) => {
                        self.workspace.search.error = Some(format!(
                            "{path} changed near line {line} since it was read; search again."
                        ));
                    }
                }
            }
            (ExcerptAsk::Written(_), Response::Done) => {
                self.workspace.search.excerpts.clear();
                self.search(SearchAction::Run);
            }
            (_, Response::Failed { message }) => self.workspace.search.error = Some(message),
            _ => {}
        }
    }

    /// Cuts from `text`, the file at `path` written in `language`, the excerpts around its lines found.
    fn cut_excerpts(&mut self, path: &str, text: &str, language: Option<&str>) {
        let search = &mut self.workspace.search;
        if !search.editing {
            return;
        }
        let found: Vec<usize> = search
            .hits
            .iter()
            .filter(|hit| hit.path == path)
            .map(|hit| hit.line)
            .collect();
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        search.excerpts.retain(|excerpt| excerpt.path != path);
        for (first, last) in excerpt_ranges(&found, lines.len()) {
            let original: String = lines[first - 1..last.min(lines.len())].concat();
            let shown = original.strip_suffix('\n').unwrap_or(&original);
            let mut document = Document::new(shown, false, false);
            document.language = language.map(str::to_owned);
            if let Some(syntax) = language
                .and_then(registry::called)
                .and_then(|grammar| Syntax::parse(grammar, shown))
            {
                document.with_syntax(syntax);
            }
            search.excerpts.push(SearchExcerpt {
                path: path.to_owned(),
                first,
                original,
                document: SharedDocument::new(document),
            });
        }
        let order: Vec<String> = search.grouped().into_iter().map(|(path, _)| path).collect();
        search.excerpts.sort_by_key(|excerpt| {
            (
                order.iter().position(|path| *path == excerpt.path),
                excerpt.first,
            )
        });
    }
}

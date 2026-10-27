//! A language's parser and the queries that say what its trees mean.

use std::fmt;
use std::sync::Arc;

use tree_sitter::{Language, Query};

use crate::engine::load_language;
use crate::kind::Kind;

/// The text of the queries a language folder holds, each empty when the folder has none.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Queries {
    /// What colour each node is: `highlights.scm`.
    pub highlights: String,
    /// Which nodes indent the lines inside them: `indents.scm`.
    pub indents: String,
    /// Which nodes are the items an outline lists: `outline.scm`.
    pub outline: String,
    /// Which stretches are written in another language: `injections.scm`.
    pub injections: String,
    /// Which tokens are brackets that pair up: `brackets.scm`.
    pub brackets: String,
    /// Which nodes are places that can be run, such as a `main` or a test: `runnables.scm`.
    pub runnables: String,
    /// Which nodes carry a lens a language folder declares, such as a service of a Compose file: `lenses.scm`.
    pub lenses: String,
}

/// How a language's files are edited.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Editing {
    /// How many columns one level of indentation is.
    pub indent: usize,
    /// Whether a level of indentation is a tab rather than spaces.
    pub tabs: bool,
    /// What starts a comment that runs to the end of the line.
    pub line_comment: Option<String>,
    /// The characters that open a pair the editor closes as the opening one is typed, each with the one that closes it.
    pub pairs: Vec<(char, char)>,
    /// The parts of a file that are edited differently from the rest.
    pub scopes: Vec<Scope>,
    /// The nodes that make up a tag, for a language whose files have tags the editor closes as they are opened.
    pub tags: Option<Tags>,
}

/// A named set of syntax nodes whose insides are edited differently from the rest of the file, such as the children of a JSX element.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Scope {
    /// What the scope is called.
    pub name: String,
    /// The kinds of the nodes it is made of.
    pub nodes: Vec<String>,
    /// What starts a comment that runs to the end of the line inside it in place of the language's; empty when it has none.
    pub line_comment: Option<String>,
    /// What opens and closes a comment inside it, used when it has no line comment.
    pub block_comment: Option<(String, String)>,
    /// Whether Emmet abbreviations are expanded inside it.
    pub emmet: Option<bool>,
}

/// The kinds of the nodes that make up a tag.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tags {
    /// The element: an opening tag, what is inside it and a closing tag.
    pub element: String,
    /// The opening tag.
    pub open: String,
    /// The closing tag.
    pub close: String,
    /// The nodes that can be the name of a tag.
    pub names: Vec<String>,
}

impl Default for Editing {
    /// Four spaces a level, no line comments, and brackets and double quotes closed as they are opened.
    fn default() -> Self {
        Self {
            indent: 4,
            tabs: false,
            line_comment: None,
            pairs: Editing::pairs_of(&default_pairs()),
            scopes: Vec::new(),
            tags: None,
        }
    }
}

/// The pairs closed as they are opened when a language names none: brackets and double quotes.
pub fn default_pairs() -> Vec<String> {
    ["()", "[]", "{}", "\"\""].map(str::to_owned).to_vec()
}

impl Editing {
    /// The pairs `written`, each two characters, the one that opens it and the one that closes it; others are left out.
    pub fn pairs_of(written: &[String]) -> Vec<(char, char)> {
        written
            .iter()
            .filter_map(|pair| {
                let mut characters = pair.chars();
                let (open, close) = (characters.next()?, characters.next()?);
                characters.next().is_none().then_some((open, close))
            })
            .collect()
    }

    /// The text of `levels` levels of indentation.
    pub fn unit(&self, levels: usize) -> String {
        if self.tabs {
            "\t".repeat(levels)
        } else {
            " ".repeat(self.indent * levels)
        }
    }
}

/// A language's parser and its queries, compiled.
pub struct Grammar {
    /// What the language is called, as its folder and the code fences that hold it name it.
    name: String,
    /// The parser's tables.
    pub(crate) language: Language,
    /// The highlights query, when the language has one.
    pub(crate) highlights: Option<Query>,
    /// The kind of each capture of the highlights query, by capture index.
    pub(crate) kinds: Vec<Option<Kind>>,
    /// The indents query, when the language has one.
    pub(crate) indents: Option<Query>,
    /// The outline query, when the language has one.
    pub(crate) outline: Option<Query>,
    /// The injections query, when the language has one.
    pub(crate) injections: Option<Query>,
    /// The brackets query, when the language has one.
    pub(crate) brackets: Option<Query>,
    /// The runnables query, when the language has one.
    pub(crate) runnables: Option<Query>,
    /// The lenses query, when the language has one.
    pub(crate) lenses: Option<Query>,
    /// How its files are edited.
    editing: Editing,
}

impl fmt::Debug for Grammar {
    /// Names the grammar.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Grammar")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The query `source` compiled for `language`, nothing when it is blank, or why it does not compile, naming it `file`.
fn compile(language: &Language, source: &str, file: &str) -> Result<Option<Query>, String> {
    if source.trim().is_empty() {
        return Ok(None);
    }
    Query::new(language, source)
        .map(Some)
        .map_err(|error| format!("{file}: {error}"))
}

impl Grammar {
    /// Loads the grammar `symbol` from the WebAssembly module `wasm` and compiles `queries` for it, calling the language `name`, whose files are edited as `editing` says.
    ///
    /// `symbol` is the name the module was built under, which its exported function `tree_sitter_<symbol>` carries.
    pub fn load(
        name: &str,
        symbol: &str,
        wasm: &[u8],
        queries: &Queries,
        editing: Editing,
    ) -> Result<Arc<Self>, String> {
        let language = load_language(symbol, wasm)?;
        let highlights = compile(&language, &queries.highlights, "highlights.scm")?;
        let kinds = highlights.as_ref().map_or_else(Vec::new, |query| {
            query
                .capture_names()
                .iter()
                .map(|name| Kind::of_capture(name))
                .collect()
        });
        Ok(Arc::new(Self {
            name: name.to_owned(),
            indents: compile(&language, &queries.indents, "indents.scm")?,
            outline: compile(&language, &queries.outline, "outline.scm")?,
            injections: compile(&language, &queries.injections, "injections.scm")?,
            brackets: compile(&language, &queries.brackets, "brackets.scm")?,
            runnables: compile(&language, &queries.runnables, "runnables.scm")?,
            lenses: compile(&language, &queries.lenses, "lenses.scm")?,
            highlights,
            kinds,
            language,
            editing,
        }))
    }

    /// What the language is called.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How its files are edited.
    pub fn editing(&self) -> &Editing {
        &self.editing
    }
}

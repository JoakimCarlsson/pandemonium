//! One text parsed with a grammar, kept up to date edit by edit: parsed again where it is drawn for as long as that is quick, and on a thread of its own when it is not.

use std::fmt;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use pm_buffer::{Change, Lines, Worker};
use tree_sitter::{InputEdit, ParseOptions, Point, Range, Tree};

use crate::engine::with_parser;
use crate::grammar::Grammar;

/// The number the next tree parsed gets, so that two parses are never mistaken for one.
static REVISIONS: AtomicU64 = AtomicU64::new(1);

/// How long parsing again after an edit may hold up the thread that edits before the rest is left to [`PARSING`].
const BUDGET: Duration = Duration::from_millis(1);

/// The thread that finishes the parses that take longer than [`BUDGET`].
static PARSING: Worker = Worker::new("syntax");

/// How a parse changed what an earlier one found, for whoever keeps something worked out from the earlier one.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Reparsed {
    /// The revision of the parse it follows.
    pub from: u64,
    /// The rows of the text before that an edit replaced, which are the rows `new_rows` now.
    pub old_rows: std::ops::Range<usize>,
    /// The rows of the text the edit made.
    pub new_rows: std::ops::Range<usize>,
    /// The rows of the text whose syntax changed besides, in order.
    pub changed: Vec<std::ops::Range<usize>>,
}

/// A tree finished on [`PARSING`].
struct Finished {
    /// The tree.
    tree: Tree,
    /// The revision it was given.
    revision: u64,
    /// How it changed what was known before it.
    reparsed: Reparsed,
}

/// A parse handed to [`PARSING`], shared by the copies of the syntax it is for.
struct Job {
    /// The tree, once it is finished.
    done: OnceLock<Finished>,
    /// Whether a later edit made it of no use.
    cancelled: AtomicBool,
}

/// A parse under way away from the thread that edits, to wait on.
#[derive(Clone)]
pub struct Parsing(Arc<Job>);

impl Parsing {
    /// Whether the parse finished or was given up.
    pub fn is_settled(&self) -> bool {
        self.0.done.get().is_some() || self.0.cancelled.load(Ordering::Relaxed)
    }

    /// A number that tells this parse apart from the others under way at the same time.
    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

/// A text parsed with a grammar.
#[derive(Clone)]
pub struct Syntax {
    /// The grammar it was parsed with.
    grammar: Arc<Grammar>,
    /// The tree of the text, or of the text before the latest edits with those edits made to it while it is parsed again away from here.
    edited: Tree,
    /// Which parse the tree came from.
    revision: u64,
    /// How the tree changed what was known before it.
    reparsed: Option<Reparsed>,
    /// The parse under way away from here for the text as it is, when there is one.
    job: Option<Arc<Job>>,
}

impl fmt::Debug for Syntax {
    /// Names the grammar and the parse.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Syntax")
            .field("grammar", &self.grammar.name())
            .field("revision", &self.revision())
            .finish()
    }
}

impl PartialEq for Syntax {
    /// Two syntaxes are the same when they came from the same parse.
    fn eq(&self, other: &Self) -> bool {
        self.revision() == other.revision()
    }
}

/// Parses `text` with `grammar`, only the stretches `ranges` of it when there are any, starting from `old`, the tree before the last edit.
pub(crate) fn parse_text(
    grammar: &Grammar,
    text: &str,
    ranges: &[Range],
    old: Option<&Tree>,
) -> Option<Tree> {
    with_parser(|parser| {
        parser.set_language(&grammar.language).ok()?;
        parser.set_included_ranges(ranges).ok()?;
        let tree = parser.parse(text, old);
        let _ = parser.set_included_ranges(&[]);
        tree
    })
    .ok()
    .flatten()
}

/// Parses `text` with `grammar` from `old`, an edited tree of it, giving up once `stop` says to; says whether it gave up, or else the tree, parsed from nothing when parsing from `old` failed.
fn parse_until(
    grammar: &Grammar,
    text: &str,
    old: &Tree,
    mut stop: impl FnMut() -> bool,
) -> Result<Option<Tree>, ()> {
    let mut halted = false;
    let tree = with_parser(|parser| {
        parser.set_language(&grammar.language).ok()?;
        let bytes = text.as_bytes();
        let mut read = |at: usize, _: Point| &bytes[at.min(bytes.len())..];
        let mut progress = |_: &tree_sitter::ParseState| {
            if stop() {
                halted = true;
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let options = ParseOptions::new().progress_callback(&mut progress);
        let tree = parser.parse_with_options(&mut read, Some(old), Some(options));
        if tree.is_none() {
            parser.reset();
        }
        tree
    })
    .ok()
    .flatten();
    match (tree, halted) {
        (Some(tree), _) => Ok(Some(tree)),
        (None, true) => Err(()),
        (None, false) => Ok(parse_text(grammar, text, &[], None)),
    }
}

/// The rows each of the stretches where `new` differs from `old` covers, in order.
fn changed_rows(old: &Tree, new: &Tree) -> Vec<std::ops::Range<usize>> {
    old.changed_ranges(new)
        .map(|range| range.start_point.row..range.end_point.row + 1)
        .collect()
}

impl Syntax {
    /// `text` parsed with `grammar`, or nothing when the parser gave up on it.
    pub fn parse(grammar: Arc<Grammar>, text: &str) -> Option<Self> {
        let tree = parse_text(&grammar, text, &[], None)?;
        Some(Self {
            grammar,
            edited: tree,
            revision: REVISIONS.fetch_add(1, Ordering::Relaxed),
            reparsed: None,
            job: None,
        })
    }

    /// The grammar the text was parsed with.
    pub fn grammar(&self) -> &Arc<Grammar> {
        &self.grammar
    }

    /// The tree of the text: the one parsed for it, or while it is parsed again away from here, the tree before with the edits since made to it.
    pub(crate) fn tree(&self) -> &Tree {
        self.finished()
            .map_or(&self.edited, |finished| &finished.tree)
    }

    /// The tree parsed away from here for the text as it is, once it is finished.
    fn finished(&self) -> Option<&Finished> {
        self.job.as_ref().and_then(|job| job.done.get())
    }

    /// Which parse the tree came from; it changes with every edit, and again when a parse away from here finishes.
    pub fn revision(&self) -> u64 {
        self.finished()
            .map_or(self.revision, |finished| finished.revision)
    }

    /// How the tree changed what the parse before it found, when it is known.
    pub fn reparsed(&self) -> Option<&Reparsed> {
        match self.finished() {
            Some(finished) => Some(&finished.reparsed),
            None => self.reparsed.as_ref(),
        }
    }

    /// The parse under way away from here for the text as it is, while it is under way.
    pub fn parsing(&self) -> Option<Parsing> {
        self.job
            .as_ref()
            .filter(|job| job.done.get().is_none())
            .map(|job| Parsing(job.clone()))
    }

    /// Takes in `change`, which turned a text whose lines `before` indexes into `after`, whose lines `after_lines` indexes, and parses again from the tree as it was.
    pub fn edit(&mut self, change: &Change, before: &Lines, after: &str, after_lines: &Lines) {
        self.edit_at(
            change,
            before.point_of(change.range.start),
            before.point_of(change.range.end),
            after_lines.point_of(change.new_end()),
            after,
        );
    }

    /// Takes in `change`, which starts at `start` and ended at `old_end` in the text before it and ends at `new_end` in `after`, the text it made, each a row and a byte column, and parses again from the tree as it was: here for as long as [`BUDGET`] allows, then away from here, the tree before with the edit made to it standing in meanwhile.
    pub fn edit_at(
        &mut self,
        change: &Change,
        start: (usize, usize),
        old_end: (usize, usize),
        new_end: (usize, usize),
        after: &str,
    ) {
        let [start, old_end, new_end] =
            [start, old_end, new_end].map(|(row, column)| Point::new(row, column));
        let from = self.revision();
        if let Some(job) = self.job.take() {
            match job.done.get() {
                Some(finished) => self.edited = finished.tree.clone(),
                None => job.cancelled.store(true, Ordering::Relaxed),
            }
        }
        self.edited.edit(&InputEdit {
            start_byte: change.range.start,
            old_end_byte: change.range.end,
            new_end_byte: change.new_end(),
            start_position: start,
            old_end_position: old_end,
            new_end_position: new_end,
        });
        let mut reparsed = Reparsed {
            from,
            old_rows: start.row..old_end.row + 1,
            new_rows: start.row..new_end.row + 1,
            changed: Vec::new(),
        };
        self.revision = REVISIONS.fetch_add(1, Ordering::Relaxed);
        let deadline = Instant::now() + BUDGET;
        match parse_until(&self.grammar, after, &self.edited, || {
            Instant::now() >= deadline
        }) {
            Ok(tree) => {
                if let Some(tree) = tree {
                    reparsed.changed = changed_rows(&self.edited, &tree);
                    self.edited = tree;
                }
            }
            Err(()) => self.parse_away(after),
        }
        self.reparsed = Some(reparsed);
    }

    /// Hands parsing `text` again from the edited tree to [`PARSING`], to be found by the copies of this syntax once it is finished.
    fn parse_away(&mut self, text: &str) {
        let job = Arc::new(Job {
            done: OnceLock::new(),
            cancelled: AtomicBool::new(false),
        });
        self.job = Some(job.clone());
        let grammar = self.grammar.clone();
        let old = self.edited.clone();
        let text = text.to_owned();
        let from = self.revision;
        PARSING.run(move || {
            if job.cancelled.load(Ordering::Relaxed) {
                return;
            }
            let cancelled = || job.cancelled.load(Ordering::Relaxed);
            if let Ok(Some(tree)) = parse_until(&grammar, &text, &old, cancelled) {
                let changed = changed_rows(&old, &tree);
                let _ = job.done.set(Finished {
                    tree,
                    revision: REVISIONS.fetch_add(1, Ordering::Relaxed),
                    reparsed: Reparsed {
                        from,
                        changed,
                        ..Reparsed::default()
                    },
                });
            } else {
                job.cancelled.store(true, Ordering::Relaxed);
            }
        });
    }
}

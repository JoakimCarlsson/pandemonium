//! The syntax colours of the code in a diff: each side read as one text by the grammar of the file's language, so what spans lines is coloured on every line it covers.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::mem::discriminant;
use std::ops::Range;
use std::sync::{Arc, LazyLock, Mutex};

use pm_treesitter::{Grammar, Syntax, registry};
use pm_ui::Mark;

use crate::sample::{DiffLine, Kind};
use crate::viewer::highlight_marks;

/// How many diffs keep their colours before all are worked out afresh.
const KEPT: usize = 128;

/// The marks of each line of a diff, by its position in the diff.
type Lines = Arc<Vec<Vec<Mark>>>;

/// The colours of the diffs coloured lately, by a fingerprint of the language and the lines.
static PAINTED: LazyLock<Mutex<HashMap<u64, Lines>>> = LazyLock::new(Default::default);

/// The syntax colours of each line of a diff, by its position in the diff; plain when the file's language has no grammar loaded.
#[derive(Clone, Default)]
pub struct DiffColours(Option<Lines>);

impl DiffColours {
    /// The colours of `diff`, the diff of a file written in `language`: removed lines as the old side reads, added and unchanged ones as the new side reads, hunk headers plain; a grammar not loaded yet is asked for, and the lines are plain until it is.
    pub fn of(diff: &[DiffLine], language: Option<&str>) -> Self {
        let Some(grammar) = language.and_then(registry::find) else {
            return Self::default();
        };
        let key = fingerprint(grammar.name(), diff);
        if let Some(painted) = PAINTED
            .lock()
            .ok()
            .and_then(|painted| painted.get(&key).cloned())
        {
            return Self(Some(painted));
        }
        let mut lines = vec![Vec::new(); diff.len()];
        paint(
            &grammar,
            diff,
            &[Kind::Remove, Kind::Context],
            &[Kind::Remove],
            &mut lines,
        );
        paint(
            &grammar,
            diff,
            &[Kind::Add, Kind::Context],
            &[Kind::Add, Kind::Context],
            &mut lines,
        );
        let painted = Arc::new(lines);
        if let Ok(mut held) = PAINTED.lock() {
            if held.len() >= KEPT {
                held.clear();
            }
            held.insert(key, painted.clone());
        }
        Self(Some(painted))
    }

    /// The marks of the line at `at`, as offsets into the text it is shown as.
    pub fn line(&self, at: usize) -> Vec<Mark> {
        self.0
            .as_ref()
            .and_then(|lines| lines.get(at).cloned())
            .unwrap_or_default()
    }
}

/// The text the code of a line of a diff is shown as: each tab four spaces.
pub fn shown(text: &str) -> String {
    text.replace('\t', "    ")
}

/// A fingerprint of the language named `language` and the lines of `diff`.
fn fingerprint(language: &str, diff: &[DiffLine]) -> u64 {
    let mut hasher = DefaultHasher::new();
    language.hash(&mut hasher);
    for line in diff {
        discriminant(&line.kind).hash(&mut hasher);
        line.text.hash(&mut hasher);
    }
    hasher.finish()
}

/// The text of the lines of `diff` of the kinds `side`, one after another as shown, and where in it each line is, by its position in the diff.
fn side_text(diff: &[DiffLine], side: &[Kind]) -> (String, Vec<(usize, Range<usize>)>) {
    let mut text = String::new();
    let mut places = Vec::new();
    for (at, line) in diff.iter().enumerate() {
        if !side.contains(&line.kind) {
            continue;
        }
        let start = text.len();
        text.push_str(&shown(&line.text));
        places.push((at, start..text.len()));
        text.push('\n');
    }
    (text, places)
}

/// Colours into `lines` the lines of `diff` of the kinds `wanted`, reading the lines of the kinds `side` as one text with `grammar`.
fn paint(
    grammar: &Arc<Grammar>,
    diff: &[DiffLine],
    side: &[Kind],
    wanted: &[Kind],
    lines: &mut [Vec<Mark>],
) {
    let (text, places) = side_text(diff, side);
    let Some(syntax) = Syntax::parse(grammar.clone(), &text) else {
        return;
    };
    let highlights = syntax.highlights(&text, 0..text.len());
    let mut first = 0;
    for (at, range) in places {
        if !wanted.contains(&diff[at].kind) {
            continue;
        }
        while highlights
            .get(first)
            .is_some_and(|highlight| highlight.range.end <= range.start)
        {
            first += 1;
        }
        lines[at] = highlight_marks(&highlights[first..], range);
    }
}

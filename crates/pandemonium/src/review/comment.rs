//! What a reader remarks on a session's diff, kept until the agent has been
//! told.
//!
//! A [`Comment`] is anchored to lines of one file, on the side of the diff
//! they are read on, and carries the text of those lines as they were when it
//! was written. The quote is what lets the comment follow its lines when the
//! file is edited above them, and what tells it its lines are gone when they
//! are rewritten. [`Comments`] is the set for one worktree: the review pane
//! and the excerpts pane both draw from the same handle, the store writes it
//! to disk, and the whole review goes to the agent as the one prompt
//! [`Comments::prompt`] builds.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::editor::OpenFile;
use crate::input::Input;

/// How many lines of context are kept either side of what was commented on.
const CONTEXT: usize = 2;

/// The shortest run of backticks a fenced block of quoted code is opened with.
const FENCE: usize = 3;

/// One comment's identity for as long as the worktree remembers it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CommentId(u64);

/// Which side of a diff a comment's lines are counted on.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Side {
    /// Lines of the file as it is in the worktree.
    New,
    /// Lines a hunk removes, counted as the last commit had them.
    Old,
}

/// Where a comment is: a file, a side and a run of lines counted from one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Anchor {
    /// The file, relative to the worktree's root.
    pub path: PathBuf,
    /// Which side of the diff the lines are counted on.
    pub side: Side,
    /// The first line commented on.
    pub first: usize,
    /// The last line commented on.
    pub last: usize,
}

/// The text of the lines a comment is on, and of those around them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Quote {
    /// Up to [`CONTEXT`] lines just before the anchored ones.
    pub before: Vec<String>,
    /// The anchored lines themselves.
    pub lines: Vec<String>,
    /// Up to [`CONTEXT`] lines just after the anchored ones.
    pub after: Vec<String>,
}

impl Quote {
    /// The quote of lines `first..=last`, read through `line`, which answers
    /// the text of a line counted from one, or nothing where there is none.
    pub fn of(line: impl Fn(usize) -> Option<String>, first: usize, last: usize) -> Self {
        let lines =
            |range: std::ops::RangeInclusive<usize>| range.filter_map(&line).collect::<Vec<_>>();
        Self {
            before: lines(first.saturating_sub(CONTEXT).max(1)..=first.saturating_sub(1)),
            lines: lines(first..=last),
            after: lines(last + 1..=last + CONTEXT),
        }
    }

    /// How many of the lines around `at` in `text` are the ones this quote
    /// kept around its own, which is how a tie between two matches is broken.
    fn agreement(&self, text: &[&str], at: usize) -> usize {
        let before = self
            .before
            .iter()
            .rev()
            .zip(text[..at].iter().rev())
            .filter(|(kept, seen)| kept.as_str() == **seen)
            .count();
        let after = self
            .after
            .iter()
            .zip(text[at + self.lines.len()..].iter())
            .filter(|(kept, seen)| kept.as_str() == **seen)
            .count();
        before + after
    }
}

/// Whether a comment is waiting to be sent, was sent, or has lost its lines.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum State {
    /// Written, and not yet sent.
    Pending,
    /// Sent to the agent in the `turn`-th review sent from this worktree.
    Sent {
        /// Which send it went in.
        turn: u64,
    },
    /// Its lines were rewritten, so where it belongs is not known.
    Outdated,
}

/// One remark on some lines of a diff.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Comment {
    /// Which comment this is.
    pub id: CommentId,
    /// The lines it is on.
    pub anchor: Anchor,
    /// The lines it is on as they were when it was written.
    pub quote: Quote,
    /// What the reader said.
    pub body: String,
    /// Whether it has been sent.
    pub state: State,
}

/// A comment being written or rewritten, and the box it is written in.
struct Draft {
    /// The comment being rewritten, or nothing for a new one.
    editing: Option<CommentId>,
    /// The lines it is on.
    anchor: Anchor,
    /// Those lines as they now are.
    quote: Quote,
    /// The box it is written in.
    input: Input,
}

/// What the pane draws of a comment being written.
#[derive(Clone)]
pub struct Composing {
    /// The comment being rewritten, or nothing for a new one.
    pub editing: Option<CommentId>,
    /// The lines it is on.
    pub anchor: Anchor,
    /// The text being written, for the pane to draw the box from.
    pub text: OpenFile,
}

/// What is written to disk of a worktree's comments.
#[derive(Default, Deserialize, Serialize)]
struct Stored {
    /// The id the next comment is given.
    next: u64,
    /// How many times a review has been sent.
    sends: u64,
    /// Every comment, in the order they were written.
    comments: Vec<Comment>,
}

/// Everything one worktree's comments are, behind the handle.
struct Set {
    /// Every comment, in the order they were written.
    comments: Vec<Comment>,
    /// The id the next comment is given.
    next: u64,
    /// How many times a review has been sent.
    sends: u64,
    /// The comment being written, while one is.
    draft: Option<Draft>,
    /// The outdated comment waiting to be put on a line of the reader's
    /// choosing.
    moving: Option<CommentId>,
    /// Whether comments already sent are drawn.
    show_sent: bool,
    /// The lines being swept over by a gesture that has not finished.
    picked: Option<Anchor>,
    /// How many times the set has changed.
    revision: u64,
    /// The revision that was last written to disk.
    saved: u64,
}

/// The comments of one worktree, shared by everything that draws them.
///
/// The handle is cheap to copy and every copy is the same set: the review
/// holds one, and the excerpts of the same worktree hold another, so a
/// comment written in one pane is the comment the other draws.
#[derive(Clone)]
pub struct Comments(Rc<RefCell<Set>>);

impl Default for Comments {
    /// No comments, with those already sent drawn.
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Set {
            comments: Vec::new(),
            next: 0,
            sends: 0,
            draft: None,
            moving: None,
            show_sent: true,
            picked: None,
            revision: 0,
            saved: 0,
        })))
    }
}

impl Comments {
    /// Takes in the comments `json` holds, as [`Comments::to_json`] wrote
    /// them, unless there are already some here.
    pub fn restore(&self, json: &str) {
        let Ok(stored) = serde_json::from_str::<Stored>(json) else {
            return;
        };
        let mut set = self.0.borrow_mut();
        if !set.comments.is_empty() || set.next > 0 {
            return;
        }
        set.comments = stored.comments;
        set.next = stored.next;
        set.sends = stored.sends;
    }

    /// The comments as they are written to disk.
    ///
    /// A worktree with none writes nothing at all.
    pub fn to_json(&self) -> String {
        let set = self.0.borrow();
        match set.comments.is_empty() {
            true => String::new(),
            false => serde_json::to_string(&Stored {
                next: set.next,
                sends: set.sends,
                comments: set.comments.clone(),
            })
            .unwrap_or_default(),
        }
    }

    /// The text to write to disk when the comments have changed since they
    /// were last written, taking it as written.
    pub fn take_unsaved(&self) -> Option<String> {
        let changed = {
            let mut set = self.0.borrow_mut();
            let changed = set.revision != set.saved;
            set.saved = set.revision;
            changed
        };
        changed.then(|| self.to_json())
    }

    /// Records that the set has changed.
    fn changed(&self) {
        self.0.borrow_mut().revision += 1;
    }

    /// How many comments are waiting to be sent.
    pub fn pending(&self) -> usize {
        self.0
            .borrow()
            .comments
            .iter()
            .filter(|comment| comment.state == State::Pending)
            .count()
    }

    /// Whether any comment has been sent.
    pub fn any_sent(&self) -> bool {
        self.0
            .borrow()
            .comments
            .iter()
            .any(|comment| matches!(comment.state, State::Sent { .. }))
    }

    /// The files that have comments on them, relative to the worktree.
    pub fn paths(&self) -> Vec<PathBuf> {
        let set = self.0.borrow();
        let paths = set
            .comments
            .iter()
            .map(|comment| comment.anchor.path.clone())
            .collect::<BTreeSet<_>>();
        paths.into_iter().collect()
    }

    /// The comment `id` names.
    pub fn get(&self, id: CommentId) -> Option<Comment> {
        self.0
            .borrow()
            .comments
            .iter()
            .find(|comment| comment.id == id)
            .cloned()
    }

    /// The comments drawn under `line` of `path` on `side`: those whose last
    /// line it is, leaving out the sent ones while they are hidden.
    pub fn ending_at(&self, path: &Path, side: Side, line: usize) -> Vec<Comment> {
        let set = self.0.borrow();
        set.comments
            .iter()
            .filter(|comment| {
                let anchor = &comment.anchor;
                comment.state != State::Outdated
                    && anchor.path == path
                    && anchor.side == side
                    && anchor.last == line
                    && set.shows(comment)
            })
            .cloned()
            .collect()
    }

    /// Every comment drawn for `path`, whether or not its lines are.
    pub fn in_file(&self, path: &Path) -> Vec<Comment> {
        let set = self.0.borrow();
        set.comments
            .iter()
            .filter(|comment| comment.anchor.path == path && set.shows(comment))
            .cloned()
            .collect()
    }

    /// Whether comments already sent are drawn.
    pub fn shows_sent(&self) -> bool {
        self.0.borrow().show_sent
    }

    /// Hides the comments already sent, or draws them again.
    pub fn toggle_sent(&self) {
        let mut set = self.0.borrow_mut();
        set.show_sent = !set.show_sent;
    }

    /// The lines a gesture is sweeping over, while one is.
    pub fn picked(&self) -> Option<Anchor> {
        self.0.borrow().picked.clone()
    }

    /// Takes down the lines a gesture is sweeping over.
    pub fn pick(&self, lines: Option<Anchor>) {
        self.0.borrow_mut().picked = lines;
    }

    /// Starts writing a new comment on `anchor`, whose lines are `quote`.
    pub fn begin(&self, anchor: Anchor, quote: Quote) {
        self.0.borrow_mut().draft = Some(Draft {
            editing: None,
            anchor,
            quote,
            input: Input::many_lines("Comment"),
        });
    }

    /// Starts rewriting the comment `id` names.
    pub fn edit(&self, id: CommentId) {
        let Some(comment) = self.get(id) else {
            return;
        };
        let mut input = Input::many_lines("Comment");
        input.set(&comment.body);
        self.0.borrow_mut().draft = Some(Draft {
            editing: Some(id),
            anchor: comment.anchor,
            quote: comment.quote,
            input,
        });
    }

    /// The comment being written, as a pane draws it.
    pub fn composing(&self) -> Option<Composing> {
        self.0.borrow().draft.as_ref().map(|draft| Composing {
            editing: draft.editing,
            anchor: draft.anchor.clone(),
            text: draft.input.text(),
        })
    }

    /// Puts the box the comment is written in through `write`, while one is.
    pub fn write<R>(&self, write: impl FnOnce(&mut Input) -> R) -> Option<R> {
        let mut set = self.0.borrow_mut();
        set.draft.as_mut().map(|draft| write(&mut draft.input))
    }

    /// Keeps what has been written, answering whether there was anything to
    /// keep.
    ///
    /// A comment that is rewritten keeps its place; one that was sent is
    /// pending again, since what the agent was told is no longer what it
    /// says. Writing nothing is not a comment.
    pub fn save(&self) -> bool {
        let Some(draft) = self.0.borrow_mut().draft.take() else {
            return false;
        };
        let body = draft.input.value().trim().to_owned();
        if body.is_empty() {
            return false;
        }
        {
            let mut set = self.0.borrow_mut();
            match draft
                .editing
                .and_then(|id| set.comments.iter_mut().find(|comment| comment.id == id))
            {
                Some(comment) => {
                    comment.body = body;
                    if matches!(comment.state, State::Sent { .. }) {
                        comment.state = State::Pending;
                    }
                }
                None => {
                    let id = CommentId(set.next);
                    set.next += 1;
                    set.comments.push(Comment {
                        id,
                        anchor: draft.anchor,
                        quote: draft.quote,
                        body,
                        state: State::Pending,
                    });
                }
            }
        }
        self.changed();
        true
    }

    /// Stops writing, throwing away what was written.
    pub fn cancel(&self) {
        self.0.borrow_mut().draft = None;
    }

    /// Takes the comment `id` names away.
    pub fn delete(&self, id: CommentId) {
        {
            let mut set = self.0.borrow_mut();
            set.comments.retain(|comment| comment.id != id);
            if set.moving == Some(id) {
                set.moving = None;
            }
        }
        self.changed();
    }

    /// Takes away every comment that has not been sent, answering whether
    /// there was any.
    pub fn discard(&self) -> bool {
        let before = self.0.borrow().comments.len();
        self.0
            .borrow_mut()
            .comments
            .retain(|comment| matches!(comment.state, State::Sent { .. }));
        let gone = self.0.borrow().comments.len() != before;
        if gone {
            self.changed();
        }
        gone
    }

    /// The outdated comment waiting for a line to be put on.
    pub fn moving(&self) -> Option<CommentId> {
        self.0.borrow().moving
    }

    /// Has the next line the reader chooses take the comment `id` names.
    pub fn arm_move(&self, id: Option<CommentId>) {
        self.0.borrow_mut().moving = id;
    }

    /// Puts the comment `id` on `anchor`, whose lines are `quote`, and makes
    /// it pending again.
    pub fn relocate(&self, id: CommentId, anchor: Anchor, quote: Quote) {
        {
            let mut set = self.0.borrow_mut();
            set.moving = None;
            let Some(comment) = set.comments.iter_mut().find(|comment| comment.id == id) else {
                return;
            };
            comment.anchor = anchor;
            comment.quote = quote;
            comment.state = State::Pending;
        }
        self.changed();
    }

    /// Follows the comments of `path` on its new side to where their lines
    /// now are in `text`, which is nothing when the file is gone.
    ///
    /// Lines that are still where they were stay put. Otherwise the quoted
    /// lines are looked for, and the match nearest the old place wins, the
    /// context kept around them breaking a tie. Lines found nowhere leave a
    /// pending comment outdated; a comment already sent keeps its place.
    pub fn reanchor(&self, path: &Path, text: Option<&str>) {
        let lines = text.map(|text| text.lines().collect::<Vec<_>>());
        let mut moved = false;
        {
            let mut set = self.0.borrow_mut();
            for comment in set
                .comments
                .iter_mut()
                .filter(|comment| comment.anchor.path == path && comment.anchor.side == Side::New)
            {
                let found = lines.as_ref().and_then(|lines| locate(comment, lines));
                match (found, comment.state) {
                    (Some((first, last)), state) => {
                        let state = match state {
                            State::Outdated => State::Pending,
                            state => state,
                        };
                        if (comment.anchor.first, comment.anchor.last) != (first, last)
                            || comment.state != state
                        {
                            comment.anchor.first = first;
                            comment.anchor.last = last;
                            comment.state = state;
                            moved = true;
                        }
                    }
                    (None, State::Pending) => {
                        comment.state = State::Outdated;
                        moved = true;
                    }
                    (None, _) => {}
                }
            }
        }
        if moved {
            self.changed();
        }
    }

    /// Keeps the comments of `path` on removed lines for as long as a hunk
    /// still removes every line they are on, `removed` being the old-side
    /// numbers of the lines the hunks remove.
    pub fn reanchor_removed(&self, path: &Path, removed: &BTreeSet<usize>) {
        let mut moved = false;
        {
            let mut set = self.0.borrow_mut();
            for comment in set
                .comments
                .iter_mut()
                .filter(|comment| comment.anchor.path == path && comment.anchor.side == Side::Old)
            {
                let held = (comment.anchor.first..=comment.anchor.last)
                    .all(|line| removed.contains(&line));
                let state = match (held, comment.state) {
                    (true, State::Outdated) => State::Pending,
                    (false, State::Pending) => State::Outdated,
                    (_, state) => state,
                };
                if state != comment.state {
                    comment.state = state;
                    moved = true;
                }
            }
        }
        if moved {
            self.changed();
        }
    }

    /// The prompt that puts every pending comment to the agent, and which
    /// comments it holds.
    ///
    /// Comments are listed by file in the order `files` gives, then by line.
    /// The code they are on is quoted in the prompt itself rather than
    /// attached, so any agent is sent the same thing whether or not it takes
    /// embedded resources.
    pub fn prompt(&self, files: &[PathBuf]) -> Option<(String, Vec<CommentId>)> {
        let set = self.0.borrow();
        let mut pending = set
            .comments
            .iter()
            .filter(|comment| comment.state == State::Pending)
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return None;
        }
        let place = |path: &Path| {
            files
                .iter()
                .position(|file| file == path)
                .unwrap_or(files.len())
        };
        pending.sort_by_key(|comment| {
            (
                place(&comment.anchor.path),
                comment.anchor.side,
                comment.anchor.first,
                comment.anchor.last,
            )
        });

        let mut prompt = String::from("Review of your changes. Address each comment.\n");
        for (at, comment) in pending.iter().enumerate() {
            prompt.push('\n');
            prompt.push_str(&entry(at + 1, comment));
        }
        Some((prompt, pending.iter().map(|comment| comment.id).collect()))
    }

    /// Marks the comments `ids` name as sent, in a send of their own.
    pub fn mark_sent(&self, ids: &[CommentId]) {
        {
            let mut set = self.0.borrow_mut();
            set.sends += 1;
            let turn = set.sends;
            for comment in set
                .comments
                .iter_mut()
                .filter(|comment| ids.contains(&comment.id))
            {
                comment.state = State::Sent { turn };
            }
        }
        self.changed();
    }
}

impl Set {
    /// Whether `comment` is drawn: sent ones are when the reader wants them.
    fn shows(&self, comment: &Comment) -> bool {
        self.show_sent || !matches!(comment.state, State::Sent { .. })
    }
}

/// The files, relative to the worktree, that the comments `json` holds are
/// on, as [`Comments::to_json`] wrote them.
pub fn paths_in(json: &str) -> Vec<PathBuf> {
    let Ok(stored) = serde_json::from_str::<Stored>(json) else {
        return Vec::new();
    };
    let paths = stored
        .comments
        .into_iter()
        .map(|comment| comment.anchor.path)
        .collect::<BTreeSet<_>>();
    paths.into_iter().collect()
}

/// Where the lines `comment` quotes now are in `text`, if they are anywhere.
fn locate(comment: &Comment, text: &[&str]) -> Option<(usize, usize)> {
    let quote = &comment.quote;
    let count = quote.lines.len();
    if count == 0 || count > text.len() {
        return None;
    }
    let matches_at = |at: usize| {
        text[at..at + count]
            .iter()
            .zip(&quote.lines)
            .all(|(seen, kept)| *seen == kept.as_str())
    };
    let old = comment.anchor.first.saturating_sub(1);
    let unmoved = old + count <= text.len()
        && comment.anchor.last + 1 - comment.anchor.first == count
        && matches_at(old);
    if unmoved {
        return Some((comment.anchor.first, comment.anchor.last));
    }
    (0..=text.len() - count)
        .filter(|at| matches_at(*at))
        .min_by_key(|at| {
            (
                at.abs_diff(old),
                std::cmp::Reverse(quote.agreement(text, *at)),
            )
        })
        .map(|at| (at + 1, at + count))
}

/// One numbered entry of the prompt: where the comment is, the code it is on
/// and what was said.
fn entry(number: usize, comment: &Comment) -> String {
    let anchor = &comment.anchor;
    let range = match anchor.first == anchor.last {
        true => anchor.first.to_string(),
        false => format!("{}-{}", anchor.first, anchor.last),
    };
    let removed = match anchor.side {
        Side::New => "",
        Side::Old => " (removed lines)",
    };
    let code = comment.quote.lines.join("\n");
    let fence = "`".repeat(fence_for(&code));
    format!(
        "{number}. {}:{range}{removed}\n{fence}{}\n{code}\n{fence}\n{}\n",
        anchor.path.display(),
        language_of(&anchor.path),
        comment.body.trim(),
    )
}

/// How many backticks fence `code` so that nothing inside it closes the fence.
fn fence_for(code: &str) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for ch in code.chars() {
        run = match ch {
            '`' => run + 1,
            _ => 0,
        };
        longest = longest.max(run);
    }
    FENCE.max(longest + 1)
}

/// The name a fenced block of `path`'s code is tagged with.
fn language_of(path: &Path) -> &str {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return "";
    };
    match extension {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "hpp" | "cc" | "cxx" => "cpp",
        "kt" => "kotlin",
        "rb" => "ruby",
        "md" => "markdown",
        "yml" => "yaml",
        other => other,
    }
}

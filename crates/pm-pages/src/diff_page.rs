//! The Diff tab of a session: the changed file chosen in the Git panel,
//! its old and new lines side by side and coloured by their syntax, with
//! notes for the agent left on its lines, the other changed files to step
//! through and buttons to stage or discard it; a conflicted file opens the
//! resolver instead.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, DiffDraft, DiffNote, DiffRow, DiffView, DiffViewVariant, IconButton,
    Label, LabelVariant, Size, Tally,
};
use pm_shell::{GitAction, LineNote, Workspace, grammar_by_ending, side_by_side};

use crate::actions::Actions;
use crate::conflict_page::conflict_page;

/// The note being written: whether it is on the new side, its line and its text.
type Draft = Option<(bool, usize, String)>;

/// The rows of `diff`, a file's diff from its first hunk on, as the diff view draws them.
pub(crate) fn diff_rows(diff: &str) -> Vec<DiffRow> {
    side_by_side(diff)
        .into_iter()
        .map(|row| DiffRow {
            left: row.left,
            right: row.right,
            hunk: row.hunk,
            left_words: row.left_words,
            right_words: row.right_words,
        })
        .collect()
}

/// Emits the Diff tab for the file `path` of `workspace`'s Git panel.
pub fn diff_page(workspace: &Workspace, path: &str, actions: &Actions) {
    let git = &workspace.git;
    if let Some(conflict) = git.conflict.as_ref().filter(|open| open.path == path) {
        conflict_page(conflict, actions);
        return;
    }
    let scheme = color_scheme();
    let listed = git.listed();
    let place = listed.iter().position(|change| change.path == path);
    let change = place.map(|place| listed[place]);
    let step = |offset: isize| {
        place
            .and_then(|place| place.checked_add_signed(offset))
            .and_then(|place| listed.get(place))
            .map(|change| change.path.clone())
    };
    let (previous, next) = (step(-1), step(1));
    let unified = remember(|| false);
    let draft = remember(|| None as Draft);
    column_with(Modifier::new().fill_max_size(), || {
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(36.0)
                .border_bottom_hairline(scheme.outline_variant)
                .padding_xy(12.0, 0.0),
            Arrangement::SpacedBy(6.0),
            Alignment::Center,
            || {
                icon(phosphor::GIT_DIFF, 13.0, scheme.on_surface_variant);
                Label::new(path.to_owned())
                    .size(Size::Xs)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                if let Some(change) = change {
                    Tally::new(change.added, change.removed).show();
                }
                if !git.notes.is_empty() {
                    let git_action = actions.git.clone();
                    Button::new(match git.notes.len() {
                        1 => "Send 1 note".to_owned(),
                        count => format!("Send {count} notes"),
                    })
                    .icon(phosphor::PAPER_PLANE_RIGHT)
                    .variant(ButtonVariant::Primary)
                    .size(Size::Xs)
                    .on_click(move || git_action(GitAction::SendNotes))
                    .show();
                }
                if let Some(change) = change {
                    let git_action = actions.git.clone();
                    let staged = change.staged;
                    let stage_path = change.path.clone();
                    Button::new(if staged { "Unstage" } else { "Stage" })
                        .icon(if staged {
                            phosphor::MINUS
                        } else {
                            phosphor::PLUS
                        })
                        .variant(ButtonVariant::Secondary)
                        .size(Size::Xs)
                        .on_click(move || {
                            git_action(GitAction::Stage(vec![stage_path.clone()], !staged))
                        })
                        .show();
                    let git_action = actions.git.clone();
                    let discard_path = change.path.clone();
                    IconButton::new(phosphor::ARROW_COUNTER_CLOCKWISE)
                        .size(Size::Xs)
                        .on_click(move || {
                            git_action(GitAction::Discard(vec![discard_path.clone()]))
                        })
                        .show();
                }
                let flip = unified.clone();
                IconButton::new(if unified.get() {
                    phosphor::COLUMNS
                } else {
                    phosphor::ROWS
                })
                .size(Size::Xs)
                .on_click(move || flip.set(!flip.get()))
                .show();
                for (glyph, target) in
                    [(phosphor::ARROW_UP, previous), (phosphor::ARROW_DOWN, next)]
                {
                    let git_action = actions.git.clone();
                    key(format!("{glyph:?}"), || {
                        IconButton::new(glyph)
                            .size(Size::Xs)
                            .on_click(move || {
                                if let Some(target) = &target {
                                    git_action(GitAction::Open(target.clone()));
                                }
                            })
                            .show();
                    });
                }
                let git_action = actions.git.clone();
                IconButton::new(phosphor::X)
                    .size(Size::Xs)
                    .on_click(move || git_action(GitAction::CloseDiff))
                    .show();
            },
        );
        let message = match change {
            None => Some("The file has no uncommitted changes any more."),
            Some(change) if change.diff.is_empty() && change.untracked => {
                Some("A new file; stage it to see its lines.")
            }
            Some(change) if change.diff.is_empty() => Some("Loading the diff…"),
            Some(_) => None,
        };
        if let Some(message) = message {
            Label::new(message)
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding(16.0))
                .show();
            return;
        }
        let rows = change
            .map(|change| diff_rows(&change.diff))
            .unwrap_or_default();
        let notes: Vec<(usize, DiffNote)> = git
            .notes
            .iter()
            .enumerate()
            .filter(|(_, note)| note.path == path)
            .map(|(place, note)| {
                (
                    place,
                    DiffNote {
                        new: note.new,
                        line: note.line,
                        text: note.text.clone(),
                    },
                )
            })
            .collect();
        let places: Rc<Vec<usize>> = Rc::new(notes.iter().map(|(place, _)| *place).collect());
        let opened = draft.clone();
        let remove = actions.git.clone();
        DiffView::new(rows)
            .variant(if unified.get() {
                DiffViewVariant::Unified
            } else {
                DiffViewVariant::Split
            })
            .grammar(grammar_by_ending(path))
            .notes(notes.into_iter().map(|(_, note)| note).collect())
            .draft(draft_of(&draft, path, actions))
            .on_line(move |new, line| opened.set(Some((new, line, String::new()))))
            .on_remove_note(move |shown| {
                if let Some(place) = places.get(shown) {
                    remove(GitAction::RemoveNote(*place));
                }
            })
            .size(Size::Sm)
            .modifier(Modifier::new().fill_max_width().weight(1.0))
            .show();
    });
}

/// The note being written on a line of the file at `path`, as the diff view takes it, when one is.
fn draft_of(draft: &State<Draft>, path: &str, actions: &Actions) -> Option<DiffDraft> {
    let (new, line, text) = draft.get()?;
    let typed = draft.clone();
    let saved = draft.clone();
    let dropped = draft.clone();
    let git_action = actions.git.clone();
    let path = path.to_owned();
    Some(DiffDraft {
        new,
        line,
        text,
        on_text: Rc::new(move |text| typed.set(Some((new, line, text)))),
        on_save: Rc::new(move || {
            if let Some((new, line, text)) = saved.get() {
                git_action(GitAction::Note(LineNote {
                    path: path.clone(),
                    line,
                    new,
                    text,
                }));
            }
            saved.set(None);
        }),
        on_cancel: Rc::new(move || dropped.set(None)),
    })
}

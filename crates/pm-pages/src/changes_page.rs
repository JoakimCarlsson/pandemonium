//! The Diff tab showing many files at once: the uncommitted changes of the
//! session's worktree, or the files a commit or a branch changed, one after
//! another, each under a header with its tally, in a list that builds only
//! the files in view and asks for the diffs of those the server listed
//! without them as they come into view.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    DiffView, FileIcon, IconButton, Label, LabelVariant, Size, Tally, TallyVariant,
};
use pm_shell::{ChangedFile, GitAction, grammar_by_ending};

use crate::actions::Actions;
use crate::diff_page::diff_rows;

/// How many files past the one coming into view have their diffs asked for with it.
const AHEAD: usize = 4;

/// How a changes page reads and what its files do.
pub struct ChangesPage<'a> {
    /// What the files are named as in the header.
    pub title: String,
    /// The files, with their diffs as far as they are loaded.
    pub files: &'a [ChangedFile],
    /// Whether the files are uncommitted ones, whose diffs are asked for as they come into view and which open on their own.
    pub working: bool,
    /// What is still on its way or went wrong, shown in place of the files.
    pub message: Option<String>,
}

/// Emits `page`.
pub fn changes_page(page: ChangesPage, actions: &Actions) {
    let scheme = color_scheme();
    let (added, removed) = page.files.iter().fold((0, 0), |(added, removed), file| {
        (added + file.added, removed + file.removed)
    });
    let working = page.working;
    column_with(Modifier::new().fill_max_size(), || {
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(36.0)
                .border_bottom_hairline(scheme.outline_variant)
                .padding_xy(12.0, 0.0),
            Arrangement::SpacedBy(pm_components::spacing(6.0)),
            Alignment::Center,
            || {
                icon(
                    if working {
                        phosphor::GIT_DIFF
                    } else {
                        phosphor::GIT_COMMIT
                    },
                    13.0,
                    scheme.on_surface_variant,
                );
                Label::new(page.title.clone())
                    .size(Size::Xs)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                Label::new(match page.files.len() {
                    1 => "1 file".to_owned(),
                    count => format!("{count} files"),
                })
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .show();
                Tally::new(added, removed).show();
                if !working {
                    let git = actions.git.clone();
                    IconButton::new(phosphor::X)
                        .size(Size::Xs)
                        .on_click(move || git(GitAction::CloseRevision))
                        .show();
                }
            },
        );
        if let Some(message) = &page.message {
            Label::new(message.clone())
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding(16.0))
                .show();
            return;
        }
        let files: Rc<Vec<ChangedFile>> = Rc::new(page.files.to_vec());
        let list = remember_lazy_list_state();
        let git = actions.git.clone();
        let count = files.len();
        lazy_list(
            Modifier::new().fill_max_width().weight(1.0),
            &list,
            count,
            move |index| {
                let file = &files[index];
                if working && file.diff.is_empty() && !file.untracked {
                    let wanted: Vec<String> = files
                        .iter()
                        .skip(index)
                        .take(AHEAD + 1)
                        .filter(|file| file.diff.is_empty() && !file.untracked)
                        .map(|file| file.path.clone())
                        .collect();
                    let load = git.clone();
                    side_effect(move || load(GitAction::LoadDiffs(wanted)));
                }
                file_block(file, working, git.clone());
            },
        );
    });
}

/// Emits `file` under a header naming it with its tally, which opens it on
/// its own when `working`, over its diff or what stands in for it.
fn file_block(file: &ChangedFile, working: bool, git: Rc<dyn Fn(GitAction)>) {
    let scheme = color_scheme();
    let path = file.path.clone();
    let mut header = Modifier::new()
        .fill_max_width()
        .height(30.0)
        .background(scheme.surface_container)
        .border_bottom_hairline(scheme.outline_variant)
        .padding_xy(12.0, 0.0);
    if working {
        header = header
            .clickable(move || git(GitAction::Open(path.clone())))
            .cursor(CursorIcon::Pointer);
    }
    column_with(Modifier::new().fill_max_width(), || {
        row_arranged(
            header,
            Arrangement::SpacedBy(pm_components::spacing(6.0)),
            Alignment::Center,
            || {
                FileIcon::new(file.name().to_owned()).size(Size::Xs).show();
                Label::new(file.name().to_owned()).size(Size::Xs).show();
                Label::new(file.folder().to_owned())
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                Tally::new(file.added, file.removed)
                    .variant(TallyVariant::Compact)
                    .show();
            },
        );
        let message = if !file.diff.is_empty() {
            None
        } else if file.untracked {
            Some("A new file; stage it to see its lines.")
        } else if working {
            Some("Loading the diff…")
        } else {
            Some("No lines changed.")
        };
        match message {
            Some(message) => Label::new(message)
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding_xy(16.0, 8.0))
                .show(),
            None => DiffView::new(diff_rows(&file.diff))
                .grammar(grammar_by_ending(&file.path))
                .scrolls(false)
                .size(Size::Sm)
                .modifier(Modifier::new().fill_max_width())
                .show(),
        }
    });
}

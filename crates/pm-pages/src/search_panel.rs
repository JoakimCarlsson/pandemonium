//! The right panel's Search tab: a field to look for text across the
//! project, switches for case and regular expressions, and the lines found
//! as excerpts of their files, each line opening its file there; or, while
//! editing, stretches of those files around the lines found in editors,
//! written back into their files on saving.

use std::collections::BTreeSet;

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, CodeEditor, Excerpt, ExcerptLine, IconButton, Label, LabelVariant, Size,
    TextInput, TextInputVariant,
};
use pm_shell::{ProjectSearch, SearchAction, SearchExcerpt, SessionTab, TabAction, Workspace};

use crate::actions::Actions;

/// Emits the Search tab for `workspace`.
pub fn search_panel(workspace: &Workspace, actions: &Actions) {
    let search = &workspace.search;
    let scheme = color_scheme();
    let folded = remember(BTreeSet::<String>::new);
    column_with(Modifier::new().fill_max_size(), || {
        row_arranged(
            Modifier::new().fill_max_width().padding_xy(8.0, 4.0),
            Arrangement::SpacedBy(pm_components::spacing(2.0)),
            Alignment::Center,
            || {
                let change = actions.search.clone();
                let run = actions.search.clone();
                TextInput::new(search.query.clone())
                    .placeholder("Search the project")
                    .variant(TextInputVariant::Search)
                    .size(Size::Sm)
                    .modifier(Modifier::new().weight(1.0))
                    .on_change(move |text| change(SearchAction::Query(text)))
                    .on_submit(move |_| run(SearchAction::Run))
                    .show();
                let case = actions.search.clone();
                IconButton::new(phosphor::TEXT_AA)
                    .size(Size::Xs)
                    .tooltip("Match case")
                    .active(search.case)
                    .on_click(move || case(SearchAction::Case))
                    .show();
                let regex = actions.search.clone();
                IconButton::new(phosphor::ASTERISK)
                    .size(Size::Xs)
                    .tooltip("Use regular expression")
                    .active(search.regex)
                    .on_click(move || regex(SearchAction::Regex))
                    .show();
                let edit = actions.search.clone();
                IconButton::new(phosphor::PENCIL_SIMPLE)
                    .size(Size::Xs)
                    .tooltip("Replace")
                    .active(search.editing)
                    .on_click(move || edit(SearchAction::Edit))
                    .show();
                let tab = actions.file_tab.clone();
                IconButton::new(phosphor::ARROW_SQUARE_OUT)
                    .size(Size::Xs)
                    .tooltip("Open in editor")
                    .on_click(move || tab(TabAction::Open(SessionTab::Excerpts)))
                    .show();
            },
        );
        Label::new(summary(search))
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .colour(if search.error.is_some() {
                scheme.error
            } else {
                scheme.on_surface_variant
            })
            .modifier(Modifier::new().padding_xy(12.0, 4.0))
            .show();
        let scroll = remember_scroll_state();
        column_arranged(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .vertical_scroll(&scroll)
                .padding_xy(4.0, 0.0),
            Arrangement::SpacedBy(pm_components::spacing(4.0)),
            Alignment::Start,
            || {
                if search.editing {
                    editable_excerpts(search, actions);
                    return;
                }
                for (path, hits) in search.grouped() {
                    let lines = hits
                        .iter()
                        .map(|hit| ExcerptLine {
                            number: hit.line,
                            text: hit.text.clone(),
                            highlight: hit.column..hit.column + hit.length,
                        })
                        .collect();
                    let expanded = !folded.with(|folded| folded.contains(&path));
                    let open = actions.open_location.clone();
                    let file = path.clone();
                    let toggled = folded.clone();
                    let flip = path.clone();
                    key(path.clone(), || {
                        Excerpt::new(path.clone(), lines)
                            .expanded(expanded)
                            .on_toggle(move || {
                                toggled.update(|folded| {
                                    if !folded.remove(&flip) {
                                        folded.insert(flip.clone());
                                    }
                                })
                            })
                            .on_open(move |line| open(file.clone(), line))
                            .show();
                    });
                }
            },
        );
    });
}

/// How tall a line of an editable excerpt is, with room for the editor's own spacing.
const EXCERPT_LINE: f32 = 18.0;

/// Emits the lines found as editable excerpts: each file's name with a
/// button writing its edits back, over an editor for each stretch of it.
pub(crate) fn editable_excerpts(search: &ProjectSearch, actions: &Actions) {
    let scheme = color_scheme();
    for (path, _) in search.grouped() {
        let excerpts: Vec<&SearchExcerpt> = search
            .excerpts
            .iter()
            .filter(|excerpt| excerpt.path == path)
            .collect();
        key(path.clone(), || {
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .border(1.0, scheme.outline_variant, 6.0)
                    .padding(4.0),
                Arrangement::SpacedBy(pm_components::spacing(4.0)),
                Alignment::Start,
                || {
                    row_arranged(
                        Modifier::new().fill_max_width().padding_xy(4.0, 2.0),
                        Arrangement::SpacedBy(pm_components::spacing(6.0)),
                        Alignment::Center,
                        || {
                            Label::new(path.clone())
                                .size(Size::Xs)
                                .modifier(Modifier::new().weight(1.0))
                                .show();
                            let save = actions.search.clone();
                            let file = path.clone();
                            Button::new("Save")
                                .icon(phosphor::FLOPPY_DISK)
                                .variant(ButtonVariant::Secondary)
                                .size(Size::Xs)
                                .disabled(excerpts.is_empty())
                                .on_click(move || save(SearchAction::Save(file.clone())))
                                .show();
                        },
                    );
                    if excerpts.is_empty() {
                        Label::new("Reading the file…")
                            .variant(LabelVariant::Subtle)
                            .size(Size::Xs)
                            .modifier(Modifier::new().padding_xy(4.0, 2.0))
                            .show();
                    }
                    for excerpt in &excerpts {
                        let lines = excerpt.document.0.borrow().lines().max(1);
                        key(excerpt.first, || {
                            let save = actions.search.clone();
                            let file = path.clone();
                            CodeEditor::new(excerpt.document.0.clone())
                                .first_line(excerpt.first)
                                .status_line(false)
                                .size(Size::Xs)
                                .on_save(move || save(SearchAction::Save(file.clone())))
                                .modifier(
                                    Modifier::new()
                                        .fill_max_width()
                                        .height(lines as f32 * EXCERPT_LINE + 8.0),
                                )
                                .show();
                        });
                    }
                },
            );
        });
    }
}

/// The line under the field: how many lines were found in how many files, or what is going on.
fn summary(search: &ProjectSearch) -> String {
    if let Some(error) = &search.error {
        return error.clone();
    }
    if search.searching.is_some() {
        return "Searching…".to_owned();
    }
    match &search.searched {
        None => "Type and press Enter to search the files of the project.".to_owned(),
        Some(_) if search.hits.is_empty() => "No results.".to_owned(),
        Some(_) => {
            let files = search.grouped().len();
            let lines = search.hits.len();
            format!(
                "{lines} result{} in {files} file{}",
                if lines == 1 { "" } else { "s" },
                if files == 1 { "" } else { "s" }
            )
        }
    }
}

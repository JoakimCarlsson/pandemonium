//! The bars a code editor opens over its text: finding and replacing, and
//! going to a line.

use pm_editor::Query;
use rustle::prelude::*;

use crate::button::{Button, ButtonVariant};
use crate::code_editor::Handle;
use crate::icon_button::IconButton;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::color_scheme;

/// Which bars are open over an editor's text.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Bars {
    /// The find bar, while it is open.
    pub(crate) find: Option<FindBar>,
    /// What is typed into the go-to-line bar, while it is open.
    pub(crate) go_to: Option<String>,
}

/// A bar a code editor can be asked to open with over its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeEditorBar {
    /// The find bar, looking for this text.
    Find(String),
    /// The find bar with its replace row, looking for this text.
    Replace(String),
    /// The go-to-line bar, with this typed.
    GoTo(String),
}

impl Bars {
    /// The bars open when an editor opens with `bar`, or none without one.
    pub(crate) fn opening(bar: Option<&CodeEditorBar>) -> Self {
        let find = |text: &str, replacing: bool| {
            let mut find = FindBar {
                replacing,
                ..FindBar::default()
            };
            find.query.text = text.to_owned();
            Some(find)
        };
        match bar {
            None => Self::default(),
            Some(CodeEditorBar::Find(text)) => Self {
                find: find(text, false),
                go_to: None,
            },
            Some(CodeEditorBar::Replace(text)) => Self {
                find: find(text, true),
                go_to: None,
            },
            Some(CodeEditorBar::GoTo(text)) => Self {
                find: None,
                go_to: Some(text.clone()),
            },
        }
    }
}

/// What the find bar holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FindBar {
    /// What is looked for.
    pub(crate) query: Query,
    /// What matches are replaced with.
    pub(crate) replacement: String,
    /// Whether the replace row shows.
    pub(crate) replacing: bool,
}

/// The modifier of a bar's row.
fn bar_row() -> Modifier {
    let scheme = color_scheme();
    Modifier::new()
        .fill_max_width()
        .height(36.0)
        .background(scheme.surface_container)
        .border_bottom_hairline(scheme.outline_variant)
        .padding_xy(10.0, 0.0)
}

/// Emits the find bar over the editor `handle` draws, holding `find`.
pub(crate) fn find_bar(handle: &Handle, find: &FindBar) {
    let (count, at, problem) = {
        let document = handle.document.borrow();
        match document.find_all(&find.query) {
            Ok(matches) => (matches.len(), document.selected_match(&matches), None),
            Err(problem) => (0, None, Some(problem)),
        }
    };
    let find = find.clone();
    let handle = handle.clone();
    column_with(Modifier::new().fill_max_width(), move || {
        row_arranged(
            bar_row(),
            Arrangement::SpacedBy(crate::spacing(6.0)),
            Alignment::Center,
            || {
                let typed = handle.clone();
                let submitted = handle.clone();
                TextInput::new(find.query.text.clone())
                    .placeholder("Find")
                    .size(Size::Sm)
                    .code()
                    .modifier(Modifier::new().width(260.0))
                    .on_change(move |text| {
                        typed.bars.update(|bars| {
                            if let Some(find) = &mut bars.find {
                                find.query.text = text.clone();
                            }
                        });
                        select_first(&typed);
                    })
                    .on_submit(move |_| step(&submitted, false))
                    .show();
                toggle(&handle, phosphor::TEXT_AA, find.query.case, |query| {
                    query.case = !query.case
                });
                toggle(
                    &handle,
                    phosphor::BRACKETS_SQUARE,
                    find.query.word,
                    |query| query.word = !query.word,
                );
                toggle(&handle, phosphor::ASTERISK, find.query.regex, |query| {
                    query.regex = !query.regex
                });
                let summary = match (&problem, count, at) {
                    (Some(problem), _, _) => problem.clone(),
                    (None, 0, _) if find.query.text.is_empty() => String::new(),
                    (None, 0, _) => "No results".to_owned(),
                    (None, count, Some(at)) => format!("{} of {count}", at + 1),
                    (None, count, None) => format!("{count} found"),
                };
                Label::new(summary)
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                let back = handle.clone();
                IconButton::new(phosphor::ARROW_UP)
                    .size(Size::Xs)
                    .on_click(move || step(&back, true))
                    .show();
                let forward = handle.clone();
                IconButton::new(phosphor::ARROW_DOWN)
                    .size(Size::Xs)
                    .on_click(move || step(&forward, false))
                    .show();
                let switch = handle.clone();
                IconButton::new(phosphor::SWAP)
                    .size(Size::Xs)
                    .active(find.replacing)
                    .on_click(move || {
                        switch.bars.update(|bars| {
                            if let Some(find) = &mut bars.find {
                                find.replacing = !find.replacing;
                            }
                        })
                    })
                    .show();
                let close = handle.clone();
                IconButton::new(phosphor::X)
                    .size(Size::Xs)
                    .on_click(move || {
                        close.bars.update(|bars| bars.find = None);
                        close.focus_text();
                    })
                    .show();
            },
        );
        if find.replacing {
            replace_row(&handle, &find);
        }
    });
}

/// Emits the replace row of the find bar holding `find`.
fn replace_row(handle: &Handle, find: &FindBar) {
    let handle = handle.clone();
    let find = find.clone();
    row_arranged(
        bar_row(),
        Arrangement::SpacedBy(crate::spacing(6.0)),
        Alignment::Center,
        move || {
            let typed = handle.clone();
            TextInput::new(find.replacement.clone())
                .placeholder("Replace")
                .size(Size::Sm)
                .code()
                .modifier(Modifier::new().width(260.0))
                .on_change(move |text| {
                    typed.bars.update(|bars| {
                        if let Some(find) = &mut bars.find {
                            find.replacement = text.clone();
                        }
                    })
                })
                .show();
            let one = handle.clone();
            Button::new("Replace")
                .variant(ButtonVariant::Secondary)
                .size(Size::Xs)
                .on_click(move || replace(&one, false))
                .show();
            let all = handle.clone();
            Button::new("Replace all")
                .variant(ButtonVariant::Secondary)
                .size(Size::Xs)
                .on_click(move || replace(&all, true))
                .show();
        },
    );
}

/// Emits a toggle of the find bar showing `glyph`, lit when `on`, which
/// changes the query with `flip`.
fn toggle(handle: &Handle, glyph: Icon, on: bool, flip: fn(&mut Query)) {
    let handle = handle.clone();
    IconButton::new(glyph)
        .size(Size::Xs)
        .active(on)
        .on_click(move || {
            handle.bars.update(|bars| {
                if let Some(find) = &mut bars.find {
                    flip(&mut find.query);
                }
            });
            select_first(&handle);
        })
        .show();
}

/// The query and replacement the find bar of `handle` holds, when it is open.
fn current(handle: &Handle) -> Option<FindBar> {
    handle.bars.get().find
}

/// Selects the first match from the caret on, as typing into the find bar does.
fn select_first(handle: &Handle) {
    let Some(find) = current(handle) else {
        return;
    };
    let edit = {
        let document = handle.document.borrow();
        let matches = document.find_all(&find.query).unwrap_or_default();
        document.first_match_edit(&matches)
    };
    handle.apply(edit);
}

/// Selects the next match, or the one before when `back`.
fn step(handle: &Handle, back: bool) {
    let Some(find) = current(handle) else {
        return;
    };
    let edit = {
        let document = handle.document.borrow();
        let matches = document.find_all(&find.query).unwrap_or_default();
        document.next_match_edit(&matches, back)
    };
    handle.apply(edit);
}

/// Replaces the selected match, or every match when `all`, then selects the next.
fn replace(handle: &Handle, all: bool) {
    let Some(find) = current(handle) else {
        return;
    };
    let edit = {
        let document = handle.document.borrow();
        let matches = document.find_all(&find.query).unwrap_or_default();
        if all {
            document.replace_all_edit(&find.query, &matches, &find.replacement)
        } else {
            document.replace_edit(&find.query, &matches, &find.replacement)
        }
    };
    handle.apply(edit);
    if !all {
        step(handle, false);
    }
}

/// Emits the go-to-line bar over the editor `handle` draws, holding `typed`.
pub(crate) fn go_to_bar(handle: &Handle, typed: &str) {
    let lines = handle.document.borrow().lines();
    let handle = handle.clone();
    let typed = typed.to_owned();
    row_arranged(
        bar_row(),
        Arrangement::SpacedBy(crate::spacing(8.0)),
        Alignment::Center,
        move || {
            let changed = handle.clone();
            let submitted = handle.clone();
            TextInput::new(typed.clone())
                .placeholder("Line number")
                .size(Size::Sm)
                .code()
                .modifier(Modifier::new().width(160.0))
                .on_change(move |text| changed.bars.update(|bars| bars.go_to = Some(text.clone())))
                .on_submit(move |text| {
                    let number = text.trim().parse::<usize>().ok();
                    submitted.bars.update(|bars| bars.go_to = None);
                    if let Some(number) = number {
                        let edit = submitted.document.borrow().go_to_line_edit(number);
                        submitted.apply([edit]);
                    }
                    submitted.focus_text();
                })
                .show();
            Label::new(format!("Go to a line from 1 to {lines}"))
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            let close = handle.clone();
            IconButton::new(phosphor::X)
                .size(Size::Xs)
                .on_click(move || {
                    close.bars.update(|bars| bars.go_to = None);
                    close.focus_text();
                })
                .show();
        },
    );
}

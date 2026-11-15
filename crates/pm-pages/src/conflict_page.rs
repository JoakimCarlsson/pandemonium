//! The resolver of a file a merge, a rebase or a cherry-pick stopped on:
//! each place where the two sides disagree, both sides shown with buttons
//! to take ours, theirs or both, beside the result, the file itself in an
//! editor, until it is marked resolved, which writes and stages it.

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, CodeEditor, IconButton, Label, LabelVariant, Size, extended_colors,
};
use pm_shell::{ConflictRegion, ConflictSide, ConflictView, GitAction, conflict_regions};

use crate::actions::Actions;

/// How many lines of a side are shown before it is cut short.
const MOST_LINES: usize = 12;

/// Emits the resolver of `conflict`.
pub fn conflict_page(conflict: &ConflictView, actions: &Actions) {
    let scheme = color_scheme();
    let extended = extended_colors();
    let text = conflict.result.0.borrow().field.text().to_owned();
    let regions = conflict_regions(&text);
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
                icon(phosphor::GIT_MERGE, 13.0, extended.warning);
                Label::new(conflict.path.clone())
                    .size(Size::Xs)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                Label::new(match regions.len() {
                    0 => "No conflicts left".to_owned(),
                    1 => "1 conflict".to_owned(),
                    count => format!("{count} conflicts"),
                })
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .show();
                for (label, side) in [
                    ("Take all ours", ConflictSide::Ours),
                    ("Take all theirs", ConflictSide::Theirs),
                ] {
                    let git = actions.git.clone();
                    key(label, || {
                        Button::new(label)
                            .variant(ButtonVariant::Secondary)
                            .size(Size::Xs)
                            .disabled(regions.is_empty())
                            .on_click(move || git(GitAction::TakeAllConflicts(side)))
                            .show();
                    });
                }
                let git = actions.git.clone();
                Button::new("Mark resolved")
                    .icon(phosphor::CHECK)
                    .variant(ButtonVariant::Primary)
                    .size(Size::Xs)
                    .disabled(!conflict.loaded || !regions.is_empty())
                    .on_click(move || git(GitAction::Resolve))
                    .show();
                let git = actions.git.clone();
                IconButton::new(phosphor::X)
                    .size(Size::Xs)
                    .on_click(move || git(GitAction::CloseDiff))
                    .show();
            },
        );
        if let Some(trouble) = &conflict.trouble {
            Label::new(trouble.clone())
                .colour(scheme.error)
                .size(Size::Xs)
                .modifier(Modifier::new().padding(16.0))
                .show();
            return;
        }
        if !conflict.loaded {
            Label::new("Reading both sides…")
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding(16.0))
                .show();
            return;
        }
        row_with(Modifier::new().fill_max_width().weight(1.0), || {
            let scroll = remember_scroll_state();
            column_arranged(
                Modifier::new()
                    .weight(1.0)
                    .fill_max_height()
                    .vertical_scroll(&scroll)
                    .padding(10.0),
                Arrangement::SpacedBy(10.0),
                Alignment::Start,
                || {
                    if regions.is_empty() {
                        Label::new(
                            "Every conflict is settled. Check the result, then mark the file resolved.",
                        )
                        .variant(LabelVariant::Subtle)
                        .size(Size::Xs)
                        .show();
                    }
                    for (place, region) in regions.iter().enumerate() {
                        key(place, || region_card(place, region, actions));
                    }
                },
            );
            spacer(
                Modifier::new()
                    .width(1.0)
                    .fill_max_height()
                    .background(scheme.outline_variant),
            );
            column_with(Modifier::new().weight(1.0).fill_max_height(), || {
                Label::new("Result")
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .modifier(Modifier::new().padding_xy(10.0, 6.0))
                    .show();
                CodeEditor::new(conflict.result.0.clone())
                    .language(conflict.language.clone())
                    .size(Size::Xs)
                    .modifier(Modifier::new().fill_max_width().weight(1.0))
                    .show();
            });
        });
    });
}

/// Emits the conflict `region`, at `place` among the file's, with both sides and the buttons settling it.
fn region_card(place: usize, region: &ConflictRegion, actions: &Actions) {
    let scheme = color_scheme();
    let extended = extended_colors();
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .background(scheme.surface_container)
            .border(1.0, scheme.outline_variant, 6.0)
            .padding(8.0),
        Arrangement::SpacedBy(6.0),
        Alignment::Start,
        || {
            Label::new(format!(
                "Conflict {} \u{b7} lines {}\u{2013}{}",
                place + 1,
                region.start + 1,
                region.end + 1
            ))
            .size(Size::Xs)
            .show();
            side_block(
                &format!("Ours \u{b7} {}", region.ours_name),
                &region.ours,
                extended.success,
            );
            side_block(
                &format!("Theirs \u{b7} {}", region.theirs_name),
                &region.theirs,
                scheme.primary,
            );
            row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(6.0),
                Alignment::Center,
                || {
                    for (label, side) in [
                        ("Take ours", ConflictSide::Ours),
                        ("Take theirs", ConflictSide::Theirs),
                        ("Take both", ConflictSide::Both),
                    ] {
                        let git = actions.git.clone();
                        key(label, || {
                            Button::new(label)
                                .variant(ButtonVariant::Secondary)
                                .size(Size::Xs)
                                .on_click(move || git(GitAction::TakeConflict(place, side)))
                                .show();
                        });
                    }
                },
            );
        },
    );
}

/// Emits one side of a conflict: its name in `tone` over its first lines.
fn side_block(name: &str, text: &str, tone: Color) {
    let scheme = color_scheme();
    let lines: Vec<&str> = text.lines().collect();
    let shown = lines
        .iter()
        .take(MOST_LINES)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    column_with(
        Modifier::new()
            .fill_max_width()
            .background(scheme.background)
            .padding_xy(6.0, 4.0),
        || {
            Label::new(name.to_owned())
                .colour(tone)
                .size(Size::Xs)
                .show();
            Label::new(if shown.is_empty() {
                "(nothing)".to_owned()
            } else {
                shown
            })
            .variant(LabelVariant::Code)
            .size(Size::Xs)
            .show();
            if lines.len() > MOST_LINES {
                Label::new(format!("{} more lines", lines.len() - MOST_LINES))
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
            }
        },
    );
}

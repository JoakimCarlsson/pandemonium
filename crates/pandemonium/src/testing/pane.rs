//! The explorer's worktree, suite, case, output and coverage rows.

use super::{Command, Store};
use crate::message::Message;
use pm_core::Scope;
use pm_ui::{Div, Styled, Theme, h_flex, text, v_flex};

/// Builds a browsable inventory and retained results in an ordinary pane.
pub fn explorer(
    theme: &Theme,
    store: &Store,
    groups: &[(Scope, String)],
    stale: impl Fn(Scope) -> bool,
) -> Div<Message> {
    let mut rows = Vec::new();
    for (scope, name) in groups {
        let scope = *scope;
        rows.push(v_flex().child(text(name).text_sm().color(theme.colors.text)));
        rows.push(
            h_flex()
                .gap(1)
                .child(button(theme, scope, "Refresh", Command::Refresh))
                .child(button(theme, scope, "Run all", Command::All))
                .child(button(theme, scope, "Cancel", Command::Cancel)),
        );
        rows.push(
            h_flex()
                .gap(1)
                .child(button(theme, scope, "Collect coverage", Command::Cover))
                .child(button(
                    theme,
                    scope,
                    "Import coverage.lcov",
                    Command::Import,
                ))
                .child(button(theme, scope, "Clear coverage", Command::Clear)),
        );
        let Some(tree) = store.worktrees.get(&scope) else {
            rows.push(
                v_flex().child(
                    text("Refresh to discover Python unittest tests")
                        .text_xs()
                        .color(theme.colors.text_muted),
                ),
            );
            continue;
        };
        if tree.discovery.is_some() {
            rows.push(
                v_flex().child(
                    text("Discovering…")
                        .text_sm()
                        .color(theme.colors.text_muted),
                ),
            );
        }
        for line in tree.error.lines() {
            rows.push(v_flex().child(text(line).text_xs().color(theme.colors.danger)));
        }
        for (index, run) in tree.runs.iter().enumerate().rev() {
            let label = format!(
                "Run {} · {} · {} cases",
                index + 1,
                run.outcome
                    .as_ref()
                    .map_or("Running".into(), |outcome| format!("{outcome:?}")),
                run.cases.len()
            );
            rows.push(
                h_flex()
                    .gap(1)
                    .child(button(theme, scope, &label, Command::History(index)))
                    .when(tree.selected_run == Some(index), |row| {
                        row.child(button(theme, scope, "Terminal output", Command::Output))
                    }),
            );
        }
        let mut suite = "";
        for (index, case) in tree.shown().iter().enumerate() {
            if suite != case.suite {
                suite = &case.suite;
                rows.push(
                    h_flex()
                        .gap(1)
                        .pl(1)
                        .child(text(suite).text_sm().color(theme.colors.text))
                        .child(button(theme, scope, "Run suite", Command::Suite(index))),
                );
            }
            let label = format!(
                "{:?} · {} · {:.3}s",
                case.status,
                case.id.rsplit('.').next().unwrap_or(&case.id),
                case.duration
            );
            rows.push(
                h_flex()
                    .gap(1)
                    .pl(2)
                    .child(button(theme, scope, &label, Command::Source(index)))
                    .child(button(theme, scope, "Run", Command::Case(index)))
                    .child(button(theme, scope, "Debug", Command::Debug(index))),
            );
        }
        if tree.shown().is_empty() && tree.discovery.is_none() {
            rows.push(
                v_flex().child(
                    text("No tests discovered")
                        .text_xs()
                        .color(theme.colors.text_muted),
                ),
            );
        }
        if let Some(index) = tree.selected_case
            && let Some(case) = tree.shown().get(index)
        {
            rows.push(
                v_flex().child(
                    text(format!("Output · {}", case.id))
                        .text_sm()
                        .color(theme.colors.text),
                ),
            );
            for line in case.output.lines() {
                rows.push(v_flex().child(text(line).text_xs().color(theme.colors.text)));
            }
        }
        if let Some(index) = tree.selected_run
            && let Some(run) = tree.runs.get(index)
            && run.outcome.is_some()
        {
            for line in run.output.lines() {
                rows.push(v_flex().child(text(line).text_xs().color(theme.colors.text_muted)));
            }
        }
        if let Some(coverage) = &tree.coverage {
            rows.push(
                v_flex().child(
                    text(if stale(scope) {
                        "Coverage · STALE (source changed; decorations hidden)"
                    } else {
                        "Coverage · current saved revision"
                    })
                    .text_sm()
                    .color(theme.colors.text),
                ),
            );
            for (index, file) in coverage.files.iter().enumerate() {
                let label = format!(
                    "{} · {}/{} lines covered",
                    file.path.display(),
                    file.covered(),
                    file.lines.len()
                );
                rows.push(button(theme, scope, &label, Command::CoverageSource(index)));
            }
        }
    }
    let offset = store.scroll.min(rows.len().saturating_sub(1));
    let mut body = v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .px(1)
        .gap(0.5);
    for row in rows.into_iter().skip(offset).take(250) {
        body = body.child(row.h_px(24.0));
    }
    body
}

/// Builds a text action that carries the originating worktree scope.
fn button(theme: &Theme, scope: Scope, label: &str, command: Command) -> Div<Message> {
    h_flex()
        .child(text(label).text_xs().color(theme.colors.text))
        .on_click(Message::Test(scope, command))
}

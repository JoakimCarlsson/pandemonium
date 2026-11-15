//! The history page: every session of every project, newest first, grouped
//! by the day it last changed and narrowed by a search.

use rustle::prelude::*;

use pm_components::{
    FilterField, Label, LabelVariant, ListRow, ListRowVariant, PageBody, PageBodyVariant,
    PageHeader, Section, SectionVariant, Size,
};
use pm_shell::{PastSession, Span, Workspace};

use crate::actions::Actions;

/// How wide the search box is.
const SEARCH_WIDTH: f32 = 500.0;

/// How wide the column naming the project is.
const PROJECT_WIDTH: f32 = 144.0;

/// How wide the column naming the machine is.
const MACHINE_WIDTH: f32 = 112.0;

/// How wide the column naming the branch is.
const BRANCH_WIDTH: f32 = 176.0;

/// How wide the column saying how long ago it was is.
const AGE_WIDTH: f32 = 80.0;

/// Emits the history page for `workspace`.
pub fn history_page(workspace: &Workspace, actions: &Actions) {
    PageHeader::new("History").show(|| {});
    PageBody::new().variant(PageBodyVariant::Full).show(|| {
        let needle = FilterField::new("Search history")
            .size(Size::Md)
            .modifier(Modifier::new().width(SEARCH_WIDTH))
            .show();
        let rows: Vec<&PastSession> = workspace
            .history
            .iter()
            .filter(|session| session.matches(&needle))
            .collect();
        if rows.is_empty() {
            let said = if workspace.history.is_empty() {
                "No sessions yet."
            } else {
                "No session matches."
            };
            Label::new(said).variant(LabelVariant::Subtle).show();
        }
        for span in Span::ALL {
            key(span.title(), || {
                let members: Vec<&PastSession> = rows
                    .iter()
                    .copied()
                    .filter(|session| session.span == span)
                    .collect();
                Section::new(span.title())
                    .variant(SectionVariant::Open)
                    .size(Size::Sm)
                    .show(members.len(), |index| history_row(members[index], actions));
            });
        }
    });
}

/// Emits the row of `session`: its title and agent, then where it ran and when.
fn history_row(session: &PastSession, actions: &Actions) {
    let reopen = actions.reopen_session.clone();
    let id = session.id;
    ListRow::new(session.title.clone())
        .note(session.agent.clone())
        .note_icon(phosphor::SPARKLE)
        .column(session.project.clone(), PROJECT_WIDTH)
        .column(session.machine.clone(), MACHINE_WIDTH)
        .column(session.branch.clone(), BRANCH_WIDTH)
        .end_column(session.age.clone(), AGE_WIDTH)
        .variant(if session.closed {
            ListRowVariant::Dimmed
        } else {
            ListRowVariant::Plain
        })
        .size(Size::Md)
        .on_click(move || reopen(id))
        .show();
}

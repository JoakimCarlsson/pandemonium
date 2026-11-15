//! The right panel: Git, Files and Search tabs, with source control shown.

use rustle::prelude::*;

use pm_components::{Size, Tab};
use pm_shell::{RightTab, Workspace};

use crate::actions::Actions;
use crate::files_panel::files_panel;
use crate::git_panel::git_panel;
use crate::search_panel::search_panel;

/// Emits the right panel for `workspace`.
pub fn side_panel(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    column_with(
        Modifier::new()
            .width(workspace.sizes.right)
            .fill_max_height()
            .background(scheme.background),
        || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(36.0)
                    .padding_xy(6.0, 0.0),
                Arrangement::SpacedBy(2.0),
                Alignment::Center,
                || {
                    for (tab, glyph) in [
                        (RightTab::Git, phosphor::GIT_BRANCH),
                        (RightTab::Files, phosphor::FOLDER),
                        (RightTab::Search, phosphor::MAGNIFYING_GLASS),
                    ] {
                        key(glyph_key(tab), || {
                            let choose = actions.right_tab.clone();
                            Tab::icon_only(glyph)
                                .size(Size::Sm)
                                .selected(workspace.right_tab == tab)
                                .on_click(move || choose(tab))
                                .show();
                        });
                    }
                },
            );
            match workspace.right_tab {
                RightTab::Git => git_panel(&workspace.git, actions),
                RightTab::Files => files_panel(workspace, actions),
                RightTab::Search => search_panel(workspace, actions),
            }
        },
    );
}

/// A stable key for `tab`.
fn glyph_key(tab: RightTab) -> u8 {
    tab as u8
}

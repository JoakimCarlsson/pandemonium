//! The bar along the bottom of the window telling what last went wrong, such
//! as a request the server refused or a lost connection, until the reader
//! dismisses it.

use rustle::prelude::*;

use pm_components::{IconButton, Label, LabelVariant, Size};
use pm_shell::{SessionCommand, Workspace};

use crate::actions::Actions;

/// Emits the status bar when `workspace` has something to tell, with a button that dismisses it through `actions`.
pub fn status_bar(workspace: &Workspace, actions: &Actions) {
    let Some(status) = &workspace.status else {
        return;
    };
    let scheme = color_scheme();
    let dismiss = actions.session.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .background(scheme.error_container)
            .padding_xy(pm_components::spacing(10.0), pm_components::spacing(3.0)),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            icon(phosphor::WARNING_CIRCLE, 14.0, scheme.on_error_container);
            Label::new(status.clone())
                .variant(LabelVariant::Body)
                .colour(scheme.on_error_container)
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            IconButton::new(phosphor::X)
                .size(Size::Xs)
                .tint(scheme.on_error_container)
                .tooltip("Dismiss")
                .on_click(move || dismiss(SessionCommand::DismissStatus))
                .show();
        },
    );
}

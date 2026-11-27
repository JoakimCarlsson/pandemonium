//! Bug reports: the notice that the app crashed the last time, the report being gathered from every server, and where it was saved.

use crate::areas::DialogEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use crate::theme::Role;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::Modal;
use crate::button::Variant;
use crate::parts::{spacer, wrapping};
use crate::state::{Outgoing, WorkspaceState};

/// How wide the dialog is, in logical pixels.
const WIDTH: f32 = 440.0;

/// How long `minutes` minutes is, in words, in the largest whole unit that fits: minutes, hours or days.
pub fn ago(minutes: u64) -> String {
    let (count, unit) = match minutes {
        0..60 => (minutes, "minute"),
        60..1440 => (minutes / 60, "hour"),
        _ => (minutes / 1440, "day"),
    };
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}

/// What the bug report dialog shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BugReport {
    /// The app crashed since the reader was last told.
    Crashed {
        /// What the crash said.
        message: String,
        /// How long ago it was, in words, such as `2 hours`; see [`ago`].
        ago: String,
    },
    /// The report is being gathered from this window and every server it reaches.
    Gathering,
    /// The report was saved as this zip on this machine.
    Saved {
        /// The zip's path.
        path: String,
    },
    /// The report could not be saved, for this reason.
    Failed {
        /// Why.
        reason: String,
    },
}

impl WorkspaceState {
    /// The state telling the reader that the app crashed `minutes` minutes ago, saying `message`, and offering to report it.
    pub fn with_crash_notice(mut self, message: String, minutes: u64) -> Self {
        self.bug_report = Some(BugReport::Crashed {
            message,
            ago: ago(minutes),
        });
        self
    }

    /// The state showing how the bug report the reader asked for stands.
    pub fn with_bug_report(mut self, report: BugReport) -> Self {
        self.bug_report = Some(report);
        self
    }

    /// Asks for a bug report to be gathered and shows that it is being gathered.
    pub fn bug_report_asked(&mut self) {
        if self.bug_report != Some(BugReport::Gathering) {
            self.bug_report = Some(BugReport::Gathering);
            self.outbox.push(Outgoing::ReportBug);
        }
    }

    /// Carries out `action` if it presses a part of the bug report dialog, and says whether it did.
    pub(crate) fn press_bug_report(&mut self, action: Target) -> bool {
        let Some(shown) = self.bug_report.as_ref() else {
            return false;
        };
        match action {
            Target::Dialog(DialogEvent::BugReportMake) => self.bug_report_asked(),
            Target::Dialog(DialogEvent::BugReportShow) => {
                if let BugReport::Saved { path } = shown {
                    self.outbox
                        .push(Outgoing::RevealReport { path: path.clone() });
                }
            }
            Target::Dialog(DialogEvent::BugReportHide | DialogEvent::BugReportOutside) => {
                self.bug_report = None
            }
            Target::Dialog(DialogEvent::BugReportInside) => {}
            _ => return false,
        }
        true
    }
}

/// Emits the bug report dialog, while there is something to show; nothing otherwise.
#[track_caller]
pub fn report_dialog(state: &WorkspaceState) {
    let Some(shown) = state.bug_report.clone() else {
        return;
    };
    let (title, words) = match &shown {
        BugReport::Crashed { message, ago } => (
            "Pandemonium crashed".to_owned(),
            format!(
                "It stopped {ago} ago with: {message}\n\nA bug report bundles the crash with this build's version and the operating system into a zip you can send. Nothing is sent by itself."
            ),
        ),
        BugReport::Gathering => (
            "Gathering a bug report".to_owned(),
            "Asking this window and every server it reaches for their version and crash log."
                .to_owned(),
        ),
        BugReport::Saved { path } => (
            "Bug report saved".to_owned(),
            format!("Saved as {path}. Attach it to the report you send."),
        ),
        BugReport::Failed { reason } => ("The bug report was not saved".to_owned(), reason.clone()),
    };
    Modal::new(
        Target::Dialog(DialogEvent::BugReportOutside),
        Target::Dialog(DialogEvent::BugReportInside),
    )
    .width(WIDTH)
    .build_with(move || {
        rustle::column_arranged(
            Modifier::new().padding(theme::sp(5)),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Start,
            || {
                rustle::basic_text(
                    Modifier::new(),
                    title,
                    theme::strong(15, Role::Text),
                    TextOverflow::Ellipsis,
                );
                wrapping(Modifier::new(), words, 13, 1.5, Role::Muted);
            },
        );
        rustle::row_arranged(
            Modifier::new().fill_max_width().padding_each(
                theme::sp(5),
                0.0,
                theme::sp(5),
                theme::sp(5),
            ),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                spacer();
                let hide = Target::Dialog(DialogEvent::BugReportHide);
                match shown {
                    BugReport::Crashed { .. } => {
                        button::action("Not now", hide, Variant::Ghost);
                        let make = Target::Dialog(DialogEvent::BugReportMake);
                        button::action("Save bug report", make, Variant::Default);
                    }
                    BugReport::Gathering | BugReport::Failed { .. } => {
                        button::action("Close", hide, Variant::Default);
                    }
                    BugReport::Saved { .. } => {
                        button::action("Close", hide, Variant::Ghost);
                        let show = Target::Dialog(DialogEvent::BugReportShow);
                        button::action("Show in folder", show, Variant::Default);
                    }
                }
            },
        );
    });
}

#[cfg(test)]
/// Tests for the bug report dialog.
mod tests {
    use super::*;
    use crate::areas::ComposerEvent;

    /// The targets of the dialog buttons `state` lays out in a window 800 by 600 pixels.
    fn laid_out_buttons(state: &WorkspaceState) -> Vec<Target> {
        let state = state.clone();
        let laid = crate::tagged::lay_out(800.0, 600.0, None, move || report_dialog(&state));
        laid.ids()
            .into_iter()
            .filter(|id| {
                [
                    Target::Dialog(DialogEvent::BugReportMake),
                    Target::Dialog(DialogEvent::BugReportShow),
                    Target::Dialog(DialogEvent::BugReportHide),
                ]
                .contains(id)
            })
            .collect()
    }

    /// A crash notice offers a report, which is asked for once, and a saved report can be shown in its folder.
    #[test]
    fn a_crash_leads_to_a_saved_report() {
        let mut state = WorkspaceState::new(&[]).with_crash_notice("boom".to_owned(), 130);
        assert!(laid_out_buttons(&state).contains(&Target::Dialog(DialogEvent::BugReportMake)));
        assert!(state.press_bug_report(Target::Dialog(DialogEvent::BugReportMake)));
        assert!(state.press_bug_report(Target::Dialog(DialogEvent::BugReportMake)));
        let asked = state.take_outbox();
        assert_eq!(
            asked
                .iter()
                .filter(|outgoing| matches!(outgoing, Outgoing::ReportBug))
                .count(),
            1
        );
        let mut state = state.with_bug_report(BugReport::Saved {
            path: "/home/me/Downloads/report.zip".to_owned(),
        });
        assert!(state.press_bug_report(Target::Dialog(DialogEvent::BugReportShow)));
        assert!(state.take_outbox().iter().any(
            |outgoing| matches!(outgoing, Outgoing::RevealReport { path } if path.ends_with("report.zip"))
        ));
        assert!(!state.press_bug_report(Target::Composer(ComposerEvent::Prompt)));
        assert!(state.press_bug_report(Target::Dialog(DialogEvent::BugReportHide)));
        assert!(laid_out_buttons(&state).is_empty());
    }
}

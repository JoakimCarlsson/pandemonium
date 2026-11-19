//! The offer, at the top of a file, to install the language it is written in when the server's machine has not got it.

use crate::areas::EditorEvent;
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use pm_protocol::ExtensionInfo;
use rustle::Modifier;

use crate::theme::Role;

use crate::button::Variant;
use crate::open_files::{Contents, Viewer};
use crate::state::{Outgoing, WorkspaceState};

/// Whether `extension` is a language written in the file `path`, by the whole name or the ending of the name.
fn reads(extension: &ExtensionInfo, path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    extension.names.iter().any(|whole| whole == name)
        || name.rsplit_once('.').is_some_and(|(stem, suffix)| {
            !stem.is_empty()
                && extension
                    .suffixes
                    .iter()
                    .any(|held| held.eq_ignore_ascii_case(suffix))
        })
}

/// The language `viewer`'s file is written in that is on offer to install, among the `extensions` there are: not installed, not left out of the list the reader picks from and not turned down.
fn offered<'a>(state: &'a WorkspaceState, viewer: &Viewer) -> Option<&'a ExtensionInfo> {
    let general = &state.window.general;
    if !general.offer_languages || viewer.head || viewer.rendering().is_some() {
        return None;
    }
    let Contents::Text(document) = &viewer.contents else {
        return None;
    };
    if document.language.is_some() {
        return None;
    }
    state.extensions.iter().find(|extension| {
        extension.kind == "language"
            && !extension.installed
            && !extension.hidden
            && !general.declined_languages.contains(&extension.id)
            && reads(extension, &viewer.path)
    })
}

/// Emits the bar at the top of `viewer`'s file that offers to install its language, or nothing when there is none on offer.
#[track_caller]
pub(crate) fn offer(state: &WorkspaceState, viewer: &Viewer) {
    let Some(extension) = offered(state, viewer) else {
        return;
    };
    let words = match (&extension.busy, &extension.problem) {
        (Some(step), _) => format!("Installing {}: {step}", extension.name),
        (None, Some(problem)) => format!("Installing {} failed: {problem}", extension.name),
        (None, None) => format!(
            "{} is not installed. Install it for highlighting and language features.",
            extension.name
        ),
    };
    let idle = extension.busy.is_none();
    let retry = extension.problem.is_some();
    kit::row(
        Modifier::new()
            .background(theme::color(Role::Surface))
            .ruled(theme::Side::Bottom)
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(1)),
        3,
        move || {
            kit::text(
                Modifier::new().weight(1.0).clip(),
                words,
                kit::line(12, Role::Muted),
            );
            if idle {
                kit::small_button(
                    if retry { "Try again" } else { "Install" },
                    Target::Editor(EditorEvent::OfferInstall),
                    Variant::Secondary,
                )
                .build();
                kit::small_button(
                    "Not now",
                    Target::Editor(EditorEvent::OfferDecline),
                    Variant::Ghost,
                )
                .build();
            }
        },
    );
}

impl WorkspaceState {
    /// The language on offer for the file shown, by folder name.
    fn offered_language(&self) -> Option<String> {
        let viewer = self.shown_viewer()?;
        offered(self, viewer).map(|extension| extension.id.clone())
    }

    /// Handles a press on the offer's buttons; false when `action` is not one of them.
    pub(crate) fn offer_pressed(&mut self, action: Target) -> bool {
        match action {
            Target::Editor(EditorEvent::OfferInstall) => {
                if let Some(id) = self.offered_language() {
                    self.outbox
                        .push(Outgoing::InstallExtensions { ids: vec![id] });
                }
            }
            Target::Editor(EditorEvent::OfferDecline) => {
                if let Some(id) = self.offered_language() {
                    self.window.general.declined_languages.push(id);
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A language read from the endings and whole names given.
    fn language(suffixes: &[&str], names: &[&str]) -> ExtensionInfo {
        ExtensionInfo {
            suffixes: suffixes.iter().map(|held| (*held).to_owned()).collect(),
            names: names.iter().map(|held| (*held).to_owned()).collect(),
            ..ExtensionInfo::default()
        }
    }

    /// A file is told by the ending of its name in any case, or by its whole name, and a bare dotfile is not an ending.
    #[test]
    fn files_are_told_by_ending_or_whole_name() {
        let shell = language(&["sh"], &["PKGBUILD"]);
        assert!(reads(&shell, "bin/run.SH"));
        assert!(reads(&shell, "pkg/PKGBUILD"));
        assert!(!reads(&shell, "bin/.sh"));
        assert!(!reads(&shell, "bin/run.py"));
    }
}

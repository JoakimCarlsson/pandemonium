//! The offer, at the top of a file, to install the language it is written in when the server's machine has not got it.

use pm_input::FocusId;
use pm_protocol::ExtensionInfo;
use pm_style::{
    Role, bg, border_b, border_color, flex, flex_none, gap, items_center, min_w_px,
    overflow_hidden, px, py,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::Variant;
use crate::open_files::{Contents, Viewer};
use crate::parts::line;
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};

/// Installs the language the offer is for.
pub const OFFER_INSTALL: FocusId = FocusId(7101);

/// Turns the offer down for the language it is for, for good.
pub const OFFER_DECLINE: FocusId = FocusId(7102);

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

/// The bar at the top of `viewer`'s file that offers to install its language, or nothing when there is none on offer.
pub(crate) fn offer(state: &WorkspaceState, viewer: &Viewer) -> Option<Node> {
    let extension = offered(state, viewer)?;
    let words = match (&extension.busy, &extension.problem) {
        (Some(step), _) => format!("Installing {}: {step}", extension.name),
        (None, Some(problem)) => format!("Installing {} failed: {problem}", extension.name),
        (None, None) => format!(
            "{} is not installed. Install it for highlighting and language features.",
            extension.name
        ),
    };
    let mut bar = node()
        .styles([
            flex(),
            items_center(),
            gap(3),
            px(4),
            py(1),
            flex_none(),
            border_b(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(line(words, 12, 1.4, Role::Muted).styles([min_w_px(0), overflow_hidden()]))
        .child(crate::parts::spacer());
    if extension.busy.is_none() {
        bar = bar
            .child(
                Button::new(
                    if extension.problem.is_some() {
                        "Try again"
                    } else {
                        "Install"
                    },
                    OFFER_INSTALL,
                )
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build(),
            )
            .child(
                Button::new("Not now", OFFER_DECLINE)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .build(),
            );
    }
    Some(bar)
}

impl WorkspaceState {
    /// The language on offer for the file shown, by folder name.
    fn offered_language(&self) -> Option<String> {
        let viewer = self.shown_viewer()?;
        offered(self, viewer).map(|extension| extension.id.clone())
    }

    /// Handles a press on the offer's buttons; false when `action` is not one of them.
    pub(crate) fn offer_pressed(&mut self, action: FocusId) -> bool {
        match action {
            OFFER_INSTALL => {
                if let Some(id) = self.offered_language() {
                    self.outbox
                        .push(Outgoing::InstallExtensions { ids: vec![id] });
                }
            }
            OFFER_DECLINE => {
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

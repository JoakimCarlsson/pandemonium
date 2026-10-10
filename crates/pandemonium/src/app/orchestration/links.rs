//! Durable session references opening ordinary agent, files and review panes.

use crate::app::App;
use crate::panes::Item;
use base64::Engine;
use pm_core::Scope;
use std::path::{Path, PathBuf};

impl App {
    /// Opens an authenticated session reference as an ordinary agent or review pane.
    pub(in crate::app) fn open_session_reference(&mut self, link: &str) -> bool {
        let Some(encoded) = link.strip_prefix("pandemonium:session/") else {
            return false;
        };
        let (encoded, view) = encoded.split_once("?view=").unwrap_or((encoded, "agent"));
        let path = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(PathBuf::from);
        let session = path.and_then(|path| {
            self.open
                .iter()
                .flat_map(|project| self.sessions.of(project.id()))
                .find(|session| session.root().stored() == path)
                .map(|session| (session.id(), session.project()))
        });
        match session {
            Some((id, project)) => {
                self.select_session(id);
                if view == "files" {
                    self.show_tool(crate::panes::Tool::Files);
                } else if view == "review" {
                    let scope = Scope::of(project, id);
                    self.show_item(self.panes.focus(), scope, Item::Review(scope), false);
                }
            }
            None => self.notices.trouble(
                "This session reference no longer exists; refresh the session list",
                None,
            ),
        }
        true
    }
}

/// Makes a durable editor reference that does not expose a transport credential.
pub(super) fn open_link(root: &Path) -> String {
    format!(
        "pandemonium:session/{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(root.to_string_lossy().as_bytes())
    )
}

//! What can be done with the file shown besides editing it: copying the selection without its shared indentation, showing the file in the file manager, starting a shell beside it, copying the address of a line on the repository's host, and going through the commits that changed it.

use pm_protocol::Location;

use crate::listing::{Items, Listing};
use crate::state::{Outgoing, WorkspaceState};

/// `selected` with the indentation its lines share taken off: the first line, which may start partway into its line, is trimmed of its own indentation and does not count towards what is shared.
pub fn trimmed(selected: &str) -> String {
    let mut lines = selected.split('\n');
    let first = lines.next().unwrap_or_default().trim_start();
    let rest: Vec<&str> = lines.collect();
    let shared = rest
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let mut out = first.to_owned();
    for line in rest {
        out.push('\n');
        out.push_str(line.get(shared..).unwrap_or_else(|| line.trim_start()));
    }
    out
}

impl WorkspaceState {
    /// The session the review panel is about and the file it shows as it is in the folder, by path.
    fn shown_file(&self) -> Option<(usize, String)> {
        let session = self.review_index()?;
        let viewer = self.shown_viewer().filter(|viewer| !viewer.head)?;
        Some((session, viewer.path.clone()))
    }

    /// Copies the selection of the file shown with the indentation its lines share taken off.
    pub fn copied_trimmed(mut self) -> Self {
        let Some(selected) = self
            .shown_document()
            .map(|document| document.field.selected().to_owned())
            .filter(|selected| !selected.is_empty())
        else {
            return self;
        };
        self.outbox.push(Outgoing::Copy {
            text: trimmed(&selected),
        });
        self
    }

    /// Shows the file shown in the file manager of this machine.
    pub fn file_revealed(mut self) -> Self {
        if let Some((session, path)) = self.shown_file() {
            self.outbox.push(Outgoing::RevealFile { session, path });
        }
        self
    }

    /// Starts a shell in the folder of the file shown and shows the dock.
    pub fn terminal_beside_file(mut self) -> Self {
        let Some((session, path)) = self.shown_file() else {
            return self;
        };
        let folder = path
            .rsplit_once('/')
            .map(|(folder, _)| folder.to_owned())
            .unwrap_or_default();
        self.open_terminal_in(session, folder);
        self
    }

    /// Copies the web address of the caret's line of the file shown on the host of its repository, at the commit checked out.
    pub fn permalink_requested(mut self) -> Self {
        let Some((session, path)) = self.shown_file() else {
            return self;
        };
        let Some(line) = self
            .shown_document()
            .map(|document| document.line_of(document.field.caret()) + 1)
        else {
            return self;
        };
        self.outbox.push(Outgoing::Permalink {
            session,
            path,
            line,
        });
        self
    }

    /// Puts `text` on the clipboard.
    pub fn with_copied(mut self, text: String) -> Self {
        self.outbox.push(Outgoing::Copy { text });
        self
    }

    /// Lists the commits that changed the file shown, at `at`.
    pub fn file_history_requested(mut self, at: (f32, f32)) -> Self {
        let Some((session, path)) = self.shown_file() else {
            return self;
        };
        self.outbox.push(Outgoing::FileHistory {
            session,
            path: path.clone(),
        });
        self.listing = Some(Listing::waiting(
            session,
            Items::History {
                path,
                commits: Vec::new(),
            },
            at,
        ));
        self
    }

    /// The state after the server listed `commits`, those that changed the file at `path` of the session at `session`.
    pub fn with_file_history(
        mut self,
        session: usize,
        path: &str,
        commits: Vec<pm_git::FileCommit>,
    ) -> Self {
        if let Some(listing) = self.listing.as_mut().filter(|held| {
            held.session == session
                && matches!(&held.items, Items::History { path: wanted, .. } if wanted == path)
        }) {
            listing.items = Items::History {
                path: path.to_owned(),
                commits,
            };
            listing.answered = true;
        }
        self
    }

    /// The state after the file of the session at `session` was read as a commit left it: `text`, written in `language`, shown read only under `label`.
    pub fn with_revision(
        self,
        session: usize,
        label: String,
        text: String,
        language: Option<String>,
    ) -> Self {
        self.go_to_location(
            session,
            Location {
                path: label,
                inside: false,
                text: Some(text),
                language,
                ..Location::default()
            },
        )
    }
}

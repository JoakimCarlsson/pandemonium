//! Telling the language servers about files made, moved and taken away.
//!
//! A server that keeps imports and module declarations in step with the
//! files they name has to hear when those files change. Before the file tree
//! moves one, every server that asked is given the chance to say what it
//! would change elsewhere — the `mod` line naming a Rust file, the imports
//! of a TypeScript one — and those changes are made first; the move waits
//! for their answers, but never for long, and never holds a frame up. After
//! a file is made, moved or taken away, the servers that asked are told.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pm_text::{Answer, Asked, Client, Indent, Position, Request, WorkspaceChange};

use crate::app::App;
use crate::tree::EditKind;

/// How long a move waits for the servers to say what it changes elsewhere.
const PATIENCE: Duration = Duration::from_secs(1);

/// A move the file tree was asked to make, waiting on the servers.
pub struct Moving {
    /// The machine and worktree owning the pending move.
    root: pm_host::Location,
    /// Where the file or folder is.
    from: PathBuf,
    /// Where it goes.
    to: PathBuf,
    /// The questions still out, and whom they were asked of.
    asked: Vec<(Arc<Client>, Asked)>,
    /// What the servers that have answered would change first.
    answers: Vec<Vec<WorkspaceChange>>,
    /// When the move goes ahead whatever is still unanswered.
    until: Instant,
}

impl App {
    /// Asks the servers over `from` what they would change before it moves
    /// to `to`, answering whether any was asked and the move is waiting.
    pub(super) fn ask_before_moving(&mut self, from: &Path, to: &Path) -> bool {
        let Some(root) = self.tree_root() else {
            return false;
        };
        let request = Request::WillRenameFiles(vec![(from.to_path_buf(), to.to_path_buf())]);
        let asked = self
            .servers_holding(&root.host, from)
            .into_iter()
            .filter(|client| client.offers(&request, from))
            .map(|client| {
                let asked = client.ask(
                    request.clone(),
                    from,
                    Position::default(),
                    Indent::default(),
                    Position::default()..Position::default(),
                );
                (client, asked)
            })
            .collect::<Vec<_>>();
        if asked.is_empty() {
            return false;
        }
        self.moving = Some(Moving {
            root,
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            asked,
            answers: Vec::new(),
            until: Instant::now() + PATIENCE,
        });
        true
    }

    /// Makes the waiting move once every server has answered or the wait is
    /// over, answering whether it was made.
    ///
    /// What the servers would change is made first, so the files that name
    /// the one moving are rewritten while it is still where they say it is.
    pub(super) fn settle_moving(&mut self) -> bool {
        let Some(moving) = self.moving.as_mut() else {
            return false;
        };
        let answers = &mut moving.answers;
        moving
            .asked
            .retain(|(client, asked)| match client.answer(*asked) {
                Some(Answer::Changes(changes)) => {
                    answers.push(changes);
                    false
                }
                Some(_) => false,
                None => true,
            });
        if !moving.asked.is_empty() && Instant::now() < moving.until {
            return false;
        }
        let Some(moving) = self.moving.take() else {
            return false;
        };
        for (client, asked) in &moving.asked {
            client.forget(*asked);
        }
        for changes in moving.answers {
            self.apply_changes_on(&moving.root.host, changes);
        }
        self.carry_out_tree_edit_on(&moving.root, EditKind::Rename, &moving.from, &moving.to);
        true
    }

    /// When the waiting move goes ahead whatever is still unanswered.
    pub(super) fn next_move(&self) -> Option<Instant> {
        self.moving.as_ref().map(|moving| moving.until)
    }

    /// Notifies this machine's language servers about created files.
    pub(super) fn tell_servers_made_on(&self, host: &pm_host::Host, paths: &[PathBuf]) {
        for (client, paths) in self.servers_of(host, paths.iter()) {
            client.did_create(&paths.into_iter().cloned().collect::<Vec<_>>());
        }
    }

    /// Notifies this machine's language servers about renamed files.
    pub(super) fn tell_servers_moved_on(&self, host: &pm_host::Host, moves: &[(PathBuf, PathBuf)]) {
        for client in self
            .servers_of(host, moves.iter().map(|(from, _)| from))
            .into_iter()
            .map(|(client, _)| client)
        {
            client.did_rename(moves);
        }
    }

    /// Notifies this machine's language servers about deleted files.
    pub(super) fn tell_servers_removed_on(&self, host: &pm_host::Host, paths: &[PathBuf]) {
        for (client, paths) in self.servers_of(host, paths.iter()) {
            client.did_delete(&paths.into_iter().cloned().collect::<Vec<_>>());
        }
    }

    /// Every server over a worktree holding any of `paths`, with the ones
    /// among them it holds.
    fn servers_of<'a>(
        &self,
        host: &pm_host::Host,
        paths: impl Iterator<Item = &'a PathBuf>,
    ) -> Vec<(Arc<Client>, Vec<&'a PathBuf>)> {
        let mut found: Vec<(Arc<Client>, Vec<&'a PathBuf>)> = Vec::new();
        for path in paths {
            for client in self.servers_holding(host, path) {
                match found
                    .iter_mut()
                    .find(|(known, _)| Arc::ptr_eq(known, &client))
                {
                    Some((_, held)) => held.push(path),
                    None => found.push((client, vec![path])),
                }
            }
        }
        found
    }

    /// The servers running over the worktree `path` is in.
    fn servers_holding(&self, host: &pm_host::Host, path: &Path) -> Vec<Arc<Client>> {
        self.scopes()
            .into_iter()
            .filter_map(|scope| self.root_of(scope))
            .filter(|root| &root.host == host && path.starts_with(root))
            .max_by_key(|root| root.as_os_str().len())
            .map(|root| self.editor.servers_over(&root))
            .unwrap_or_default()
    }
}

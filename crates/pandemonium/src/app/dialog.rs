//! The platform's own file and folder pickers, asked without stopping the
//! window.
//!
//! A picker stays open for as long as the reader takes to choose, and a
//! window that waits on it stops drawing for all that time: its agents'
//! answers pile up unread and its terminals stand still. So the picker is
//! awaited on a thread of its own, and what was chosen is taken in when that
//! thread wakes the window.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::agent::TalkId;
use crate::app::{App, Wake};

/// What the reader chose in a picker, and what it was chosen for.
pub(super) enum Chosen {
    /// A folder to open as a project.
    Project(PathBuf),
    /// A folder to clone the repository at the address into.
    CloneInto(String, PathBuf),
    /// Files to attach to a conversation's next prompt.
    Attach(TalkId, Vec<PathBuf>),
}

/// The choices made in pickers and not yet taken in.
pub(super) type Choices = Arc<Mutex<Vec<Chosen>>>;

impl App {
    /// Asks for a folder to open as a project.
    pub(super) fn ask_project(&self) {
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Open a folder")
            .pick_folder();
        self.ask_later(async move {
            dialog
                .await
                .map(|folder| Chosen::Project(folder.path().to_path_buf()))
        });
    }

    /// Asks for the folder to clone the repository at `url` into.
    pub(super) fn ask_clone_into(&self, url: String) {
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Clone into")
            .pick_folder();
        self.ask_later(async move {
            dialog
                .await
                .map(|folder| Chosen::CloneInto(url, folder.path().to_path_buf()))
        });
    }

    /// Asks for files to attach to the next prompt of `session`.
    pub(super) fn ask_attachments(&self, session: TalkId) {
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Attach files")
            .pick_files();
        self.ask_later(async move {
            let files = dialog.await?;
            Some(Chosen::Attach(
                session,
                files.iter().map(|file| file.path().to_path_buf()).collect(),
            ))
        });
    }

    /// Waits for `asked` on a thread of its own, and wakes the window with
    /// what was chosen.
    fn ask_later(&self, asked: impl Future<Output = Option<Chosen>> + Send + 'static) {
        let choices = self.choices.clone();
        let wake = self.waker(Wake::Chosen);
        std::thread::spawn(move || {
            if let Some(chosen) = pollster::block_on(asked)
                && let Ok(mut choices) = choices.lock()
            {
                choices.push(chosen);
                drop(choices);
                wake();
            }
        });
    }

    /// Takes in every choice made in a picker, answering whether any was.
    pub(super) fn take_chosen(&mut self) -> bool {
        let chosen = self
            .choices
            .lock()
            .map(|mut choices| std::mem::take(&mut *choices))
            .unwrap_or_default();
        let any = !chosen.is_empty();
        for chosen in chosen {
            match chosen {
                Chosen::Project(root) => {
                    let _ = self.open.find_or_open(root);
                    self.read_new_worktrees();
                    self.store();
                }
                Chosen::CloneInto(url, under) => self.clone_into(url, under),
                Chosen::Attach(session, paths) => {
                    if let Some(talk) = self.agents.get_mut(session) {
                        for path in paths {
                            talk.attach_file(path);
                        }
                    }
                    self.focus_prompt(session);
                }
            }
        }
        any
    }
}

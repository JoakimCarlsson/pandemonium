//! Going back in a session: asking its agent what a turn changed in the files, to put them back as they were before a turn and to take a turn's changes back out of them, and taking the conversation back to before a turn.
//!
//! The harness keeps the files before and after each turn and does the work; the server asks it, waits for its answer and tells the clients.

use pm_harness::{Event, FileNews};

use crate::conversation;
use crate::core::{Session, Told};

impl Session {
    /// Whether the harness keeps the files of this session before and after each turn, which it does in a git repository.
    pub(crate) fn keeps_files(&self) -> bool {
        pm_git::is_repository(&self.directory)
    }

    /// The id the harness keeps the files of the reader's turn `turn` under.
    ///
    /// # Errors
    ///
    /// Says that no files were kept before the turn when there is no such turn or its files were not kept.
    fn kept_turn(&self, turn: u64) -> Result<String, String> {
        conversation::turns(&self.log)
            .get(turn as usize)
            .filter(|asked| asked.checkpoint)
            .map(|asked| asked.saved.clone())
            .ok_or_else(|| format!("no files were kept before turn {}", turn + 1))
    }

    /// Starts the agent of this session, numbered `number`, when it is not running, or runs as another login than the session's, so that it can be asked about files, and returns what that noted.
    fn ready_agent(&mut self, number: u64) -> Told {
        if self.running_as != self.meta.login {
            self.chat = None;
        }
        if self.chat.is_some() {
            return Told::new();
        }
        let told = self.drop_trust(number);
        let settings = if crate::sessions::has_history(number) {
            Vec::new()
        } else {
            conversation::settings(&self.log)
        };
        self.start_agent(number, &settings);
        told
    }

    /// Asks the agent of this session, numbered `number`, what the reader's turn `turn` changed in the files, starting it when it is not running; the answer comes to [`Session::take_files`].
    ///
    /// # Errors
    ///
    /// Says that no files were kept before the turn.
    pub(crate) fn ask_changes(&mut self, number: u64, turn: u64) -> Result<Told, String> {
        let id = self.kept_turn(turn)?;
        let told = self.ready_agent(number);
        if let Some(chat) = &self.chat {
            chat.changes(&id);
        }
        Ok(told)
    }

    /// Asks the agent of this session, numbered `number`, to put the files back as they were before the reader's turn `turn`, starting it when it is not running; the answer comes to [`Session::take_files`].
    ///
    /// # Errors
    ///
    /// Fails while the agent is on a turn, and when no files were kept before the turn.
    pub(crate) fn ask_restore(&mut self, number: u64, turn: u64) -> Result<Told, String> {
        self.idle()?;
        let id = self.kept_turn(turn)?;
        let told = self.ready_agent(number);
        if let Some(chat) = &self.chat {
            chat.restore(&id);
        }
        Ok(told)
    }

    /// Asks the agent of this session, numbered `number`, to take what the reader's turn `turn` changed back out of the files `paths`, or of every file it changed when there are none, starting it when it is not running; the answer comes to [`Session::take_files`].
    ///
    /// # Errors
    ///
    /// Fails while the agent is on a turn, and when no files were kept before the turn.
    pub(crate) fn ask_undo(
        &mut self,
        number: u64,
        turn: u64,
        paths: &[String],
    ) -> Result<Told, String> {
        self.idle()?;
        let id = self.kept_turn(turn)?;
        let told = self.ready_agent(number);
        if let Some(chat) = &self.chat {
            chat.undo(&id, paths);
        }
        Ok(told)
    }

    /// The reader's turn the harness saved under the id `id`, by how many of the reader's turns came before it, when the conversation still holds it.
    fn turn_saved_as(&self, id: &str) -> Option<u64> {
        conversation::turns(&self.log)
            .iter()
            .position(|turn| turn.saved == id)
            .map(|position| position as u64)
    }

    /// Takes in what the agent of this session, numbered `number`, answered about the files of a turn: notes files put back, or a turn's changes taken out, in the log, hands the answer to the request that has waited longest for one, and returns what was noted.
    pub(crate) fn take_files(&mut self, number: u64, news: FileNews) -> Told {
        let event = match &news {
            FileNews::Restored { turn, files } => {
                self.turn_saved_as(turn).map(|turn| Event::Restored {
                    turn,
                    files: files.clone(),
                })
            }
            FileNews::Undone { turn, files } if !files.is_empty() => {
                self.turn_saved_as(turn).map(|turn| Event::Undone {
                    turn,
                    files: files.clone(),
                })
            }
            _ => None,
        };
        let told = event
            .map(|event| vec![(number, self.note(number, &event), event)])
            .unwrap_or_default();
        if let Some(waiter) = self.file_waiters.pop_front() {
            let _ = waiter.send(news);
        }
        told
    }

    /// Takes the conversation of this session, numbered `number`, back to before the reader's turn `turn`: its agent stops, dropping the trust request it waited on, and the next prompt starts one afresh that is told what came before.
    pub(crate) fn rewind(&mut self, number: u64, turn: u64) -> Result<Told, String> {
        self.idle()?;
        if turn as usize >= conversation::turns(&self.log).len() {
            return Err(format!("session {number} has no turn {turn}"));
        }
        self.chat = None;
        let first = conversation::turns(&self.log)
            .get(turn as usize)
            .map(|held| held.saved.clone());
        let mut told = self.drop_queue(number);
        told.extend(self.drop_trust(number));
        let event = Event::Rewound { turn };
        told.push((number, self.note(number, &event), event));
        self.resume_before = first;
        Ok(told)
    }

    /// Nothing when the agent of this session is between turns, and why going back has to wait when it is on one.
    fn idle(&self) -> Result<(), String> {
        match self.busy {
            true => Err("the agent is on a turn; stop it first".to_owned()),
            false => Ok(()),
        }
    }
}

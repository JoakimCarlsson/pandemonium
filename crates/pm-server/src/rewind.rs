//! Going back in a session: keeping its files before each turn, putting them back, and taking the conversation back to before a turn.

use pm_acp::Event;

use crate::conversation;
use crate::core::{Session, Told};

/// The reference keeping the files of session `number` as they were before the turn whose prompt is numbered `seq` in its log.
fn kept(number: u64, seq: u64) -> String {
    format!("refs/worktree/pandemonium/sessions/{number}/turns/{seq}")
}

/// The reference keeping the files of session `number` as they were before they were last put back.
fn backup(number: u64) -> String {
    format!("refs/worktree/pandemonium/sessions/{number}/before-restore")
}

impl Session {
    /// Keeps the files of this session, numbered `number`, as they are before the turn whose prompt will be numbered `seq`, and says whether they were kept.
    pub(crate) fn checkpoint(&self, number: u64, seq: u64) -> bool {
        pm_git::is_repository(&self.directory)
            && pm_git::checkpoint(&self.directory, &kept(number, seq)).is_ok()
    }

    /// Takes the conversation of this session, numbered `number`, back to before the reader's turn `turn`: its agent stops, and the next prompt starts one afresh that is told what came before.
    pub(crate) fn rewind(&mut self, number: u64, turn: u64) -> Result<Told, String> {
        self.idle()?;
        if turn as usize >= conversation::turns(&self.log).len() {
            return Err(format!("session {number} has no turn {turn}"));
        }
        self.chat = None;
        let mut told = self.drop_queue(number);
        let event = Event::Rewound { turn };
        told.push((number, self.note(number, &event), event));
        Ok(told)
    }

    /// Puts the files of this session, numbered `number`, back as they were before the reader's turn `turn`, keeping them as they are first.
    pub(crate) fn restore(&mut self, number: u64, turn: u64) -> Result<Told, String> {
        self.idle()?;
        let turns = conversation::turns(&self.log);
        let Some(before) = turns.get(turn as usize).filter(|before| before.checkpoint) else {
            return Err(format!("no files were kept before turn {turn}"));
        };
        let files = pm_git::restore(&self.directory, &kept(number, before.seq), &backup(number))
            .map_err(|reason| format!("could not put the files back: {reason}"))?;
        let event = Event::Restored { turn, files };
        Ok(vec![(number, self.note(number, &event), event)])
    }

    /// Nothing when the agent of this session is between turns, and why going back has to wait when it is on one.
    fn idle(&self) -> Result<(), String> {
        match self.busy {
            true => Err("the agent is on a turn; stop it first".to_owned()),
            false => Ok(()),
        }
    }
}

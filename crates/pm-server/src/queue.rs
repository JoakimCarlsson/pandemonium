//! Prompts that wait for the agent's turn to end, and the one place a prompt is handed to an agent.

use pm_harness::Event;

use crate::conversation;
use crate::core::{Session, Told};
use crate::sessions;

/// A prompt a reader wrote, with its files stored where the agent can read them.
pub(crate) struct Prompt {
    /// What the reader wrote.
    pub(crate) text: String,
    /// The files attached, by the path they are stored at.
    pub(crate) paths: Vec<String>,
    /// The names of the files attached.
    pub(crate) names: Vec<String>,
}

/// A prompt waiting for the agent's turn to end.
pub(crate) struct Waiting {
    /// The number it goes by within its session.
    pub(crate) id: u64,
    /// The prompt.
    pub(crate) prompt: Prompt,
}

impl Session {
    /// Hands `prompt` to the agent of this session, numbered `number`, or queues it behind the turn the agent is on; with `now`, stops that turn and puts the prompt first. A question asked on the side is handed over at once, turn or not.
    pub(crate) fn submit(&mut self, number: u64, prompt: Prompt, now: bool) -> Told {
        if let Some(question) = pm_harness::aside_question(&prompt.text) {
            let question = question.to_owned();
            crate::uploads::discard(&prompt.paths);
            return self.ask_aside(number, &question);
        }
        if !self.busy {
            return self.hand(number, prompt);
        }
        if steers_goal(&prompt.text)
            && let Some(chat) = &self.chat
        {
            chat.prompt(&prompt.text, &prompt.paths, self.next_turn_id(), "");
            return Told::new();
        }
        let id = self.next_queued;
        self.next_queued += 1;
        let event = Event::Queued {
            id,
            text: prompt.text.clone(),
            files: prompt.names.clone(),
        };
        let waiting = Waiting { id, prompt };
        if now {
            self.queue.push_front(waiting);
            self.interrupt();
        } else {
            self.queue.push_back(waiting);
        }
        vec![(number, self.note(number, &event), event)]
    }

    /// Puts the waiting prompt `id` of this session, numbered `number`, first and stops the turn so it is handed over next. Returns nothing when no prompt waits under `id`.
    pub(crate) fn hurry(&mut self, number: u64, id: u64) -> Option<Told> {
        let position = self.queue.iter().position(|waiting| waiting.id == id)?;
        let waiting = self.queue.remove(position)?;
        self.queue.push_front(waiting);
        if self.busy {
            self.interrupt();
            Some(Told::new())
        } else {
            Some(self.advance(number))
        }
    }

    /// Removes the waiting prompt `id` and its uploads, or does nothing if it has already left the queue.
    pub(crate) fn remove_queued(&mut self, number: u64, id: u64) -> Told {
        let Some(position) = self.queue.iter().position(|waiting| waiting.id == id) else {
            return Told::new();
        };
        let Some(waiting) = self.queue.remove(position) else {
            return Told::new();
        };
        crate::uploads::discard(&waiting.prompt.paths);
        let event = Event::Unqueued(id);
        vec![(number, self.note(number, &event), event)]
    }

    /// Replaces the text of the waiting prompt `id` of this session, numbered `number`, with `text`, keeping its place, its id and its files, and announces it again under the same id; then lets the queue go on if `holder` held it. Changes nothing when the prompt has already left the queue, or when `text` is empty and no files go with it.
    pub(crate) fn edit_queued(
        &mut self,
        number: u64,
        holder: usize,
        id: u64,
        text: String,
    ) -> Told {
        let mut told = self.replace_queued(number, id, text);
        told.extend(self.release_queue(number, holder));
        told
    }

    /// Replaces the text of the waiting prompt `id` with `text` and announces it, or does nothing when no such prompt waits or nothing would be left of it.
    fn replace_queued(&mut self, number: u64, id: u64, text: String) -> Told {
        let Some(waiting) = self.queue.iter_mut().find(|waiting| waiting.id == id) else {
            return Told::new();
        };
        if text.trim().is_empty() && waiting.prompt.paths.is_empty() {
            return Told::new();
        }
        waiting.prompt.text = text;
        let event = Event::Queued {
            id,
            text: waiting.prompt.text.clone(),
            files: waiting.prompt.names.clone(),
        };
        vec![(number, self.note(number, &event), event)]
    }

    /// Holds the queue of this session, numbered `number`, for the client `holder`, or lets it go on when `held` is not set and `holder` is the one that holds it. A queue another client holds stays held. Letting go hands the agent the prompt that has waited longest if it is between turns.
    pub(crate) fn hold_queue(&mut self, number: u64, holder: usize, held: bool) -> Told {
        if held {
            self.held_by.get_or_insert(holder);
            return Told::new();
        }
        if self.held_by != Some(holder) {
            return Told::new();
        }
        self.held_by = None;
        self.advance(number)
    }

    /// Lets the queue of this session, numbered `number`, go on if `holder` held it, as that client went away.
    pub(crate) fn release_queue(&mut self, number: u64, holder: usize) -> Told {
        self.hold_queue(number, holder, false)
    }

    /// Hands the agent of this session, numbered `number`, the prompt that has waited longest, once its turn is over. Hands nothing over while a client holds the queue.
    pub(crate) fn advance(&mut self, number: u64) -> Told {
        if self.busy || self.held_by.is_some() {
            return Told::new();
        }
        let Some(waiting) = self.queue.pop_front() else {
            return Told::new();
        };
        let left = Event::Unqueued(waiting.id);
        let mut told = vec![(number, self.note(number, &left), left)];
        told.extend(self.hand(number, waiting.prompt));
        told
    }

    /// Drops every prompt waiting in this session, numbered `number`.
    pub(crate) fn drop_queue(&mut self, number: u64) -> Told {
        let waiting: Vec<Waiting> = self.queue.drain(..).collect();
        waiting
            .into_iter()
            .map(|waiting| {
                crate::uploads::discard(&waiting.prompt.paths);
                let event = Event::Unqueued(waiting.id);
                (number, self.note(number, &event), event)
            })
            .collect()
    }

    /// Ends in the log of this session, numbered `number`, the prompts it says are waiting, since a server that starts again holds none, and numbers new ones after them.
    pub(crate) fn forget_queue(&mut self, number: u64) {
        let mut waiting: Vec<u64> = Vec::new();
        for (_, event) in &self.log {
            match event {
                Event::Queued { id, .. } => {
                    if !waiting.contains(id) {
                        waiting.push(*id);
                    }
                    self.next_queued = self.next_queued.max(id + 1);
                }
                Event::Unqueued(id) => waiting.retain(|held| held != id),
                _ => {}
            }
        }
        for id in waiting {
            self.note(number, &Event::Unqueued(id));
        }
    }

    /// Opens the session again if it was closed, keeps its files as they are, starts its agent if it is not running and hands it `prompt` as its next turn.
    ///
    /// An agent started afresh is told the conversation so far, and any agent is told of files put back since its last turn. An agent whose login changed, is started afresh. A trust request left from an agent that stopped is dropped before a new one starts, since the new one asks again.
    fn hand(&mut self, number: u64, prompt: Prompt) -> Told {
        if self.meta.closed {
            self.meta.closed = false;
            sessions::save_meta(number, &self.meta);
        }
        if self.running_as != self.meta.login {
            self.chat = None;
        }
        let afresh = self.chat.is_none();
        let mut told = if afresh {
            self.drop_trust(number)
        } else {
            Told::new()
        };
        let preface =
            conversation::preface(&self.log, afresh && !crate::sessions::has_history(number));
        let turn = self.next_turn_id();
        let event = Event::Prompted {
            text: prompt.text.clone(),
            files: prompt.names,
            checkpoint: self.keeps_files(),
            turn,
            saved: None,
        };
        told.push((number, self.note(number, &event), event));
        if afresh {
            let settings = if crate::sessions::has_history(number) {
                Vec::new()
            } else {
                conversation::settings(&self.log)
            };
            self.start_agent(number, &settings);
        }
        if let Some(chat) = &self.chat {
            chat.prompt(&prompt.text, &prompt.paths, turn, &preface);
            self.busy = true;
        }
        told
    }

    /// Asks the agent of this session, numbered `number`, `question` on the side, starting the agent when it is not running: the question is neither queued nor noted as a turn, a turn the agent is on goes on, and the answer comes as an aside.
    fn ask_aside(&mut self, number: u64, question: &str) -> Told {
        let told = if self.chat.is_none() {
            let told = self.drop_trust(number);
            let settings = if crate::sessions::has_history(number) {
                Vec::new()
            } else {
                conversation::settings(&self.log)
            };
            self.start_agent(number, &settings);
            told
        } else {
            Told::new()
        };
        if let Some(chat) = &self.chat {
            chat.btw(question);
        }
        told
    }

    /// Stops the turn the agent is on, so that what waits first is handed over when it ends.
    fn interrupt(&self) {
        if let Some(chat) = &self.chat {
            chat.cancel();
        }
    }
}

/// Whether `text` is a `/goal` command the agent carries out at once while it works, starting no turn: showing the goal, pausing, clearing or budgeting it. Setting and resuming it start a turn, so they wait like any prompt.
pub(crate) fn steers_goal(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    matches!(
        words.as_slice(),
        ["/goal"] | ["/goal", "pause" | "clear"] | ["/goal", "budget", _]
    )
}

#[cfg(test)]
mod tests;

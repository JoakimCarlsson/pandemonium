//! Prompts that wait for the agent's turn to end, and the one place a prompt is handed to an agent.

use pm_acp::Event;

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
    /// Hands `prompt` to the agent of this session, numbered `number`, or queues it behind the turn the agent is on; with `now`, stops that turn and puts the prompt first.
    pub(crate) fn submit(&mut self, number: u64, prompt: Prompt, now: bool) -> Told {
        if !self.busy {
            return self.hand(number, prompt);
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

    /// Hands the agent of this session, numbered `number`, the prompt that has waited longest, once its turn is over.
    pub(crate) fn advance(&mut self, number: u64) -> Told {
        if self.busy {
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
        let ids: Vec<u64> = self.queue.drain(..).map(|waiting| waiting.id).collect();
        ids.into_iter()
            .map(|id| {
                let event = Event::Unqueued(id);
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
                    waiting.push(*id);
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

    /// Opens the session again if it was closed, starts its agent if it is not running and hands it `prompt` as its next turn.
    fn hand(&mut self, number: u64, prompt: Prompt) -> Told {
        if self.meta.closed {
            self.meta.closed = false;
            sessions::save_meta(number, &self.meta);
        }
        let event = Event::Prompted {
            text: prompt.text.clone(),
            files: prompt.names,
        };
        let told = vec![(number, self.note(number, &event), event)];
        if self.chat.is_none() {
            self.start_agent(&[]);
        }
        if let Some(chat) = &self.chat {
            chat.prompt(&prompt.text, &prompt.paths);
            self.busy = true;
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

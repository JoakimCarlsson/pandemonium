//! What has been said in one conversation, in the order it was said.
//!
//! An agent streams: a sentence arrives as a dozen runs of text, and a tool
//! call is announced before it is titled and titled before it has run. What a
//! reader wants is neither — it is the conversation as it now stands, which is
//! what this holds. Runs of one voice join into one block, and a tool call
//! replaces the block it is a later word about.

use pm_acp::{Step, ToolCall, Voice};

/// One thing the conversation shows.
#[derive(Clone, Debug)]
pub enum Block {
    /// A passage of text, of whichever voice said it.
    Said(Voice, String),
    /// A tool call, as it now stands.
    Ran(ToolCall),
    /// The plan the agent is working to.
    Planned(Vec<Step>),
    /// Something the editor has to say about the conversation itself.
    Note(String),
}

/// Everything one conversation has come to.
#[derive(Default)]
pub struct Transcript {
    /// The blocks, oldest first.
    blocks: Vec<Block>,
}

impl Transcript {
    /// Everything said so far, oldest first.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Adds a run of text, joining it to the one before it where it belongs.
    ///
    /// Two runs of the same voice with nothing between them are one passage:
    /// the agent broke it up to send it, not to have it read that way.
    pub fn say(&mut self, voice: Voice, text: &str) {
        match self.blocks.last_mut() {
            Some(Block::Said(said, passage)) if *said == voice => passage.push_str(text),
            _ => self.blocks.push(Block::Said(voice, text.to_owned())),
        }
    }

    /// Adds a tool call, or replaces the one it is a later word about.
    pub fn ran(&mut self, call: ToolCall) {
        match self
            .blocks
            .iter_mut()
            .rev()
            .find(|block| matches!(block, Block::Ran(ran) if ran.id == call.id))
        {
            Some(block) => *block = Block::Ran(call),
            None => self.blocks.push(Block::Ran(call)),
        }
    }

    /// Puts the plan in place of the one before it.
    ///
    /// A plan is one thing the agent keeps rewriting, not a series of them:
    /// the reader is shown where it has got to, once.
    pub fn planned(&mut self, steps: Vec<Step>) {
        match self
            .blocks
            .iter_mut()
            .find(|block| matches!(block, Block::Planned(_)))
        {
            Some(block) => *block = Block::Planned(steps),
            None => self.blocks.push(Block::Planned(steps)),
        }
    }

    /// Adds something the editor itself has to say.
    pub fn note(&mut self, note: impl Into<String>) {
        self.blocks.push(Block::Note(note.into()));
    }
}

//! What has been said in one conversation, in the order it was said.
//!
//! An agent streams: a sentence arrives as a dozen runs of text, and a tool
//! call is announced before it is titled and titled before it has run. What a
//! reader wants is neither — it is the conversation as it now stands, which is
//! what this holds. Runs of one voice join into one block, and a tool call
//! replaces the block it is a later word about.

use base64::Engine;
use pm_acp::{Step, ToolCall, Voice};
use pm_gfx::Image;

use crate::image::{Decodes, Decoding};

/// One thing the conversation shows.
#[derive(Clone, Debug)]
pub enum Block {
    /// A passage of text, of whichever voice said it.
    Said(Voice, String),
    /// An image the reader attached to a prompt.
    Picture(Image),
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
    /// The revision each block was last changed at, in the same order.
    stamps: Vec<u64>,
    /// How many changes the conversation has taken, which is the revision
    /// the last change was stamped with.
    revision: u64,
    /// Images decoded from old session messages, by block and position.
    pictures: Decodes<(usize, usize)>,
}

impl Transcript {
    /// Everything said so far, oldest first.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The revision each block was last changed at, in the order of
    /// [`Transcript::blocks`]; a block whose stamp is unchanged reads as it did.
    pub fn stamps(&self) -> &[u64] {
        &self.stamps
    }

    /// The revision of the conversation as a whole, which changes whenever
    /// any block of it does.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Adds `block` after the last one.
    fn push(&mut self, block: Block) {
        self.blocks.push(block);
        self.stamps.push(0);
        self.stamp(self.blocks.len() - 1);
    }

    /// Marks the block at `at` as changed.
    fn stamp(&mut self, at: usize) {
        self.revision += 1;
        self.stamps[at] = self.revision;
    }

    /// Adds a run of text, joining it to the one before it where it belongs.
    ///
    /// Two runs of the same voice with nothing between them are one passage:
    /// the agent broke it up to send it, not to have it read that way.
    pub fn say(&mut self, voice: Voice, text: &str) {
        match self.blocks.last_mut() {
            Some(Block::Said(said, passage)) if *said == voice => {
                passage.push_str(text);
                self.stamp(self.blocks.len() - 1);
            }
            _ => self.push(Block::Said(voice, text.to_owned())),
        }
    }

    /// Adds a sent image beside the reader's prompt.
    pub fn picture(&mut self, image: Image) {
        self.push(Block::Picture(image));
    }

    /// Where an image echoed by a loaded session has got to; the first
    /// asking starts decoding it away from the window, and one that failed
    /// stays failed rather than being tried again.
    pub fn restored_picture(&self, block: usize, place: usize, source: &str) -> Decoding {
        let key = (block, place);
        if let Some(decoding) = self.pictures.peek(&key) {
            return decoding;
        }
        let encoded = source
            .split_once(";base64,")
            .map_or(source, |(_, data)| data)
            .trim_matches(['\'', '"'])
            .to_owned();
        self.pictures.start(key, move || {
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| error.to_string())
        });
        Decoding::Pending
    }

    /// Adds a tool call, or replaces the one it is a later word about.
    pub fn ran(&mut self, call: ToolCall) {
        match self
            .blocks
            .iter()
            .rposition(|block| matches!(block, Block::Ran(ran) if ran.id == call.id))
        {
            Some(at) => self.replace(at, Block::Ran(call)),
            None => self.push(Block::Ran(call)),
        }
    }

    /// Puts the plan in place of the one before it.
    ///
    /// A plan is one thing the agent keeps rewriting, not a series of them:
    /// the reader is shown where it has got to, once.
    pub fn planned(&mut self, steps: Vec<Step>) {
        match self
            .blocks
            .iter()
            .position(|block| matches!(block, Block::Planned(_)))
        {
            Some(at) => self.replace(at, Block::Planned(steps)),
            None => self.push(Block::Planned(steps)),
        }
    }

    /// Adds something the editor itself has to say.
    pub fn note(&mut self, note: impl Into<String>) {
        self.push(Block::Note(note.into()));
    }

    /// Puts `block` in place of the one at `at`.
    fn replace(&mut self, at: usize, block: Block) {
        self.blocks[at] = block;
        self.stamp(at);
    }
}

//! What has been said in one conversation, in the order it was said.
//!
//! An agent streams: a sentence arrives as a dozen runs of text, and a tool
//! call is announced before it is titled and titled before it has run. What a
//! reader wants is neither — it is the conversation as it now stands, which is
//! what this holds. Runs of one voice join into one block, and a tool call
//! replaces the block it is a later word about.

use std::collections::BTreeMap;
use std::time::Instant;

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
    Ran(Box<ToolCall>),
    /// The plan the agent is working to.
    Planned(Vec<Step>),
    /// Something the editor has to say about the conversation itself.
    Note(String),
    /// A failed turn and whether the agent offered a compact command.
    Failure(String, bool),
}

/// Everything one conversation has come to.
#[derive(Default)]
pub struct Transcript {
    /// The blocks, oldest first.
    blocks: Vec<Block>,
    /// Native message identities attached to text blocks.
    messages: BTreeMap<usize, String>,
    /// Subagent calls attached to an already known parent.
    children: BTreeMap<String, Vec<ToolCall>>,
    /// Start and optional finish of each thought passage.
    thoughts: BTreeMap<usize, (Instant, Option<Instant>)>,
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
        self.finish_thought();
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
        self.say_identified(voice, text, None);
    }

    /// The native provider identity of a text block, when the adapter supplies one.
    pub fn message_id(&self, block: usize) -> Option<&str> {
        self.messages.get(&block).map(String::as_str)
    }

    /// Adds streamed text without merging distinct provider messages.
    pub fn say_identified(&mut self, voice: Voice, text: &str, message: Option<String>) {
        if voice != Voice::Thought {
            self.finish_thought();
        }
        let streaming = voice != Voice::Thought
            || self
                .thoughts
                .last_key_value()
                .is_none_or(|(_, (_, finished))| finished.is_none());
        let same_message = message.as_deref().is_none_or(|message| {
            self.messages
                .get(&self.blocks.len().saturating_sub(1))
                .is_none_or(|last| last == message)
        });
        match self.blocks.last_mut() {
            Some(Block::Said(said, passage)) if *said == voice && streaming && same_message => {
                passage.push_str(text);
                self.stamp(self.blocks.len() - 1);
            }
            _ => {
                self.push(Block::Said(voice, text.to_owned()));
                if voice == Voice::Thought {
                    self.thoughts
                        .insert(self.blocks.len() - 1, (Instant::now(), None));
                }
            }
        }
        if let Some(message) = message {
            self.messages.insert(self.blocks.len() - 1, message);
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
        self.finish_thought();
        if let Some(at) = self
            .blocks
            .iter()
            .rposition(|block| matches!(block, Block::Ran(ran) if ran.id == call.id))
        {
            self.replace(at, Block::Ran(Box::new(call)));
            return;
        }
        if let Some(parent) = self.children.iter().find_map(|(parent, children)| {
            children
                .iter()
                .any(|child| child.id == call.id)
                .then(|| parent.clone())
        }) {
            let children = self.children.get_mut(&parent).unwrap();
            let child = children
                .iter_mut()
                .find(|child| child.id == call.id)
                .unwrap();
            *child = call;
            self.stamp_parent(&parent);
            return;
        }
        if let Some(parent) = call
            .parent
            .as_ref()
            .filter(|parent| self.has_call(parent))
            .cloned()
        {
            self.children.entry(parent.clone()).or_default().push(call);
            self.stamp_parent(&parent);
        } else {
            self.push(Block::Ran(Box::new(call)));
        }
    }

    /// Calls directly attached to a parent, in arrival order.
    pub fn children(&self, parent: &str) -> &[ToolCall] {
        self.children.get(parent).map_or(&[], Vec::as_slice)
    }

    /// Whether a parent has already appeared in this transcript.
    fn has_call(&self, id: &str) -> bool {
        self.blocks
            .iter()
            .any(|block| matches!(block, Block::Ran(call) if call.id == id))
            || self.children.values().flatten().any(|call| call.id == id)
    }

    /// Marks the top-level card containing a changed child.
    fn stamp_parent(&mut self, parent: &str) {
        if let Some(at) = self
            .blocks
            .iter()
            .position(|block| matches!(block, Block::Ran(call) if call.id == parent))
        {
            self.stamp(at);
        } else if let Some(ancestor) = self.children.iter().find_map(|(ancestor, children)| {
            children
                .iter()
                .any(|child| child.id == parent)
                .then(|| ancestor.clone())
        }) {
            self.stamp_parent(&ancestor);
        }
    }

    /// The timing of a thought passage, recorded when its events arrive.
    pub fn thought(&self, at: usize) -> Option<(Instant, Option<Instant>)> {
        self.thoughts.get(&at).copied()
    }

    /// Freezes the latest streaming thought when activity moves on.
    pub fn finish_thought(&mut self) {
        if let Some((&at, (_, finished))) = self.thoughts.last_key_value()
            && finished.is_none()
        {
            self.thoughts.get_mut(&at).unwrap().1 = Some(Instant::now());
            self.stamp(at);
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

    /// Adds an agent failure and its available recovery action.
    pub fn failure(&mut self, message: String, compact: bool) {
        self.push(Block::Failure(message, compact));
    }

    /// Puts `block` in place of the one at `at`.
    fn replace(&mut self, at: usize, block: Block) {
        self.blocks[at] = block;
        self.stamp(at);
    }
}

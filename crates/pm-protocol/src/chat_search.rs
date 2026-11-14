//! What is said about the conversations a search of a server's chats found.

use serde::{Deserialize, Serialize};

/// Where in a conversation the words looked for were found.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatField {
    /// The title of the session.
    #[default]
    Title,
    /// A prompt the reader sent.
    Prompt,
    /// Something the agent said.
    Reply,
}

/// One session whose title or conversation holds what was looked for.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatHit {
    /// The session, by the server's number.
    pub session: u64,
    /// Where the best match was found.
    pub field: ChatField,
    /// A short stretch of the text that matched, around the match, on one line.
    pub preview: String,
    /// How well it matched, higher being better; hits come best first.
    pub score: u32,
}

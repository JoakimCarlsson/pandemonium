//! The pipe to one language server, and the thread that alone writes to it.
//!
//! A write to a server blocks for as long as the server is not reading, and
//! a server busy writing its own answers is not reading. Were the window to
//! write, one server that fills both pipes at once would stop the window
//! with it. So nobody else writes: the window and the reader thread hand
//! their messages over, and only this thread ever waits on the pipe.

use std::io::BufWriter;
use std::process::ChildStdin;
use std::sync::mpsc::{self, Receiver, Sender};

use ropey::Rope;
use serde_json::{Value, json};

use crate::frame;

/// One message on its way to a server, as it was handed over.
pub(super) enum Outgoing {
    /// A message whose body is already built.
    Message(Value),
    /// A file's whole new text, built into a `didChange` only as it is
    /// written.
    ///
    /// The rope is the buffer's own, shared rather than copied, so telling a
    /// server about a keystroke costs the window nothing the size of the
    /// file.
    Change {
        /// The file, as the server names it.
        uri: String,
        /// The version the text is at.
        version: i32,
        /// The text itself.
        text: Rope,
    },
}

impl Outgoing {
    /// Whether this is about the document at `uri`.
    pub(super) fn is_about(&self, uri: &str) -> bool {
        match self {
            Self::Message(message) => message["params"]["textDocument"]["uri"] == uri,
            Self::Change { uri: changed, .. } => changed == uri,
        }
    }

    /// Whether this is a whole-text change, which a later one makes moot.
    pub(super) fn is_change(&self) -> bool {
        matches!(self, Self::Change { .. })
    }

    /// The message as it goes on the wire.
    fn into_message(self) -> Value {
        match self {
            Self::Message(message) => message,
            Self::Change { uri, version, text } => json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri, "version": version },
                    "contentChanges": [{ "text": text.to_string() }],
                },
            }),
        }
    }
}

/// Where messages for one server are handed over, for its writer to send.
#[derive(Clone)]
pub(super) struct Outbox(Sender<Outgoing>);

impl Outbox {
    /// Starts the thread that writes to `stdin`, answering where it is fed.
    ///
    /// The thread lasts as long as anyone can still hand it a message, or
    /// until the pipe breaks, and closes the server's input as it ends.
    pub(super) fn start(stdin: ChildStdin) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || write(stdin, receiver));
        Self(sender)
    }

    /// Hands `outgoing` over without waiting, dropping it if the server has
    /// gone.
    pub(super) fn send(&self, outgoing: Outgoing) {
        let _ = self.0.send(outgoing);
    }
}

/// Writes everything handed over to `stdin`, until nothing more can come or
/// the pipe has gone.
fn write(stdin: ChildStdin, receiver: Receiver<Outgoing>) {
    let mut stdin = BufWriter::new(stdin);
    for outgoing in receiver {
        if frame::write(&mut stdin, &outgoing.into_message()).is_err() {
            return;
        }
    }
}

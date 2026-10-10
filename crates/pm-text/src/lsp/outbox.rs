//! The pipe to one language server, and the thread that alone writes to it.
//!
//! A write to a server blocks for as long as the server is not reading, and
//! a server busy writing its own answers is not reading. Were the window to
//! write, one server that fills both pipes at once would stop the window
//! with it. So nobody else writes: the window and the reader thread hand
//! their messages over, and only this thread ever waits on the pipe.

use pm_host::Input as ChildStdin;
use std::io::BufWriter;
use std::sync::mpsc::{self, Receiver, Sender};

use lsp_types::notification::DidChangeTextDocument;
use lsp_types::{DidChangeTextDocumentParams, Uri, VersionedTextDocumentIdentifier};
use ropey::Rope;
use serde_json::Value;

use crate::frame;
use crate::lsp::encoding::Encoding;
use crate::lsp::log::Log;
use crate::lsp::{rpc, sync};

/// One message on its way to a server, as it was handed over.
pub(super) enum Outgoing {
    /// A message whose body is already built.
    Message(Value),
    /// A file's new text, built into a `didChange` only as it is written.
    ///
    /// The ropes are the buffer's own, shared rather than copied, so telling
    /// a server about a keystroke costs the window nothing the size of the
    /// file: what changed between them is worked out on the writer thread.
    Change {
        /// The file, as the server names it.
        uri: Uri,
        /// The version the text is at.
        version: i32,
        /// The text the server was last told, when it asked to be told only
        /// what changed since.
        before: Option<Rope>,
        /// The text as it now stands.
        after: Rope,
        /// How the server counts a column, for the span that changed.
        encoding: Encoding,
    },
}

impl Outgoing {
    /// Whether this is about the document at `uri`.
    pub(super) fn is_about(&self, uri: &Uri) -> bool {
        match self {
            Self::Message(message) => message["params"]["textDocument"]["uri"] == uri.as_str(),
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
            Self::Change {
                uri,
                version,
                before,
                after,
                encoding,
            } => rpc::notification::<DidChangeTextDocument>(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier::new(uri, version),
                content_changes: vec![match before {
                    Some(before) => sync::ranged(&before, &after, encoding),
                    None => sync::whole(&after),
                }],
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
    ///
    /// Everything written goes to `log` as well while the protocol is traced.
    pub(super) fn start(stdin: ChildStdin, log: Log) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || write(stdin, receiver, log));
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
fn write(stdin: ChildStdin, receiver: Receiver<Outgoing>, log: Log) {
    let mut stdin = BufWriter::new(stdin);
    for outgoing in receiver {
        let message = outgoing.into_message();
        log.trace("-->", &message);
        if frame::write(&mut stdin, &message).is_err() {
            return;
        }
    }
}

//! The wire: numbered messages to an adapter, written in the order they are
//! sent.
//!
//! Everything the editor says to an adapter goes through one queue, whoever
//! says it — the window asking to step, the reader thread answering the
//! adapter's handshake — so two messages never interleave on the pipe and
//! the numbers they carry never repeat. What is sent before the adapter has
//! been reached waits in the queue until it is.

use std::io::{BufReader, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

use pm_text::frame;
use serde_json::{Value, json};

/// The way messages go to one adapter.
#[derive(Clone)]
pub(crate) struct Wire {
    /// The queue the writer drains onto the pipe.
    outbox: Sender<Value>,
    /// The number the next message will carry.
    next: Arc<AtomicI64>,
}

impl Wire {
    /// A wire whose messages go to `outbox`.
    pub(crate) fn new(outbox: Sender<Value>) -> Self {
        Self {
            outbox,
            next: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Sends the request `command` with `arguments`, and says the number it
    /// went under.
    pub(crate) fn request(&self, command: &str, arguments: Value) -> i64 {
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let _ = self.outbox.send(json!({
            "seq": seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        }));
        seq
    }

    /// Refuses the adapter's own request numbered `request_seq`, which asked
    /// for `command`.
    ///
    /// An adapter asks its client for a few things — a terminal to run the
    /// program in, a second session for a child process — and the editor
    /// does none of them yet. A refusal is an answer, which lets the adapter
    /// go on without it rather than wait for one that never comes.
    pub(crate) fn refuse(&self, request_seq: i64, command: &str) {
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let _ = self.outbox.send(json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "command": command,
            "success": false,
            "message": "not supported by this editor",
        }));
    }
}

/// Writes what `outbox` is sent onto `sink`, until the wire is dropped or the
/// pipe has gone.
pub(crate) fn write_all(outbox: Receiver<Value>, mut sink: impl Write) {
    while let Ok(message) = outbox.recv() {
        if frame::write(&mut sink, &message).is_err() {
            return;
        }
    }
}

/// Reads frames from `source` and hands each to `dispatch`, until the pipe
/// closes.
pub(crate) fn read_all(source: impl Read, mut dispatch: impl FnMut(&Value)) {
    let mut source = BufReader::new(source);
    while let Ok(Some(message)) = frame::read(&mut source) {
        dispatch(&message);
    }
}

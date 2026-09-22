//! The wire: JSON-RPC messages, one per line.
//!
//! This is the whole of what the protocol looks like on a pipe — a message
//! is a line of JSON, and a line that is not JSON is not a message. Nothing
//! above this module reads or writes a byte of it.

use std::io::{self, BufRead, Write};

use serde_json::Value;

/// Writes `message` as one line.
pub fn write(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    writer.write_all(&body)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Reads one message, or `None` once the pipe has closed.
///
/// An agent is a program before it is a peer, and programs print things: a
/// banner, a warning, a stray newline. A line that is not a JSON object is
/// skipped rather than fatal, and the next one is read instead.
pub fn read(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if let Ok(message @ Value::Object(_)) = serde_json::from_str(line.trim()) {
            return Ok(Some(message));
        }
    }
}

//! The wire: JSON messages in `Content-Length` frames.
//!
//! This is the whole of what a language server and a debug adapter look like
//! on a pipe — a header block, a blank line and a JSON body. The two
//! protocols say different things in those bodies and frame them the same
//! way, so both of their clients read and write through here and nothing
//! else reads or writes a byte of it.

use std::io::{self, BufRead, Write};

use serde_json::Value;

/// The header a frame states its length in.
const LENGTH: &str = "content-length:";

/// Writes `message` as one frame.
pub fn write(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// Reads one frame, or `None` once the pipe has closed.
///
/// A frame whose body is not JSON is skipped rather than fatal: a peer that
/// writes a malformed message is still a peer worth listening to.
pub fn read(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix(LENGTH) {
            length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(length) = length else {
        return Ok(None);
    };
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body).ok())
}

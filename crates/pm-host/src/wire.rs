//! Length delimited multiplexed frames; only control payloads are JSON.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Read, Write};

/// A request with JSON control data.
pub const REQUEST: u8 = 1;
/// A JSON reply to a request.
pub const REPLY: u8 = 2;
/// Raw stream data.
pub const BYTES: u8 = 3;
/// A closed byte stream.
pub const EOF: u8 = 4;
/// A JSON watch notification.
pub const EVENT: u8 = 5;
/// Acknowledges consumption of one raw or event frame.
pub const CREDIT: u8 = 6;
/// The largest frame accepted from a peer.
const LIMIT: usize = 64 * 1024 * 1024;

/// One message on a logical channel.
pub struct Frame {
    /// The payload's interpretation.
    pub kind: u8,
    /// The request or stream identity.
    pub channel: u32,
    /// The bytes carried by the message.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Encodes a control payload.
    pub fn json(kind: u8, channel: u32, value: &impl Serialize) -> io::Result<Self> {
        Ok(Self {
            kind,
            channel,
            payload: serde_json::to_vec(value)?,
        })
    }

    /// Reads one bounded frame, returning clean EOF between frames.
    pub fn read(reader: &mut impl Read) -> io::Result<Option<Self>> {
        let mut length = [0; 4];
        match reader.read(&mut length[..1])? {
            0 => return Ok(None),
            _ => reader.read_exact(&mut length[1..])?,
        }
        let length = u32::from_be_bytes(length) as usize;
        if !(5..=LIMIT).contains(&length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid host frame length",
            ));
        }
        let mut header = [0; 5];
        reader.read_exact(&mut header)?;
        let mut payload = vec![0; length - 5];
        reader.read_exact(&mut payload)?;
        Ok(Some(Self {
            kind: header[0],
            channel: u32::from_be_bytes(header[1..].try_into().unwrap()),
            payload,
        }))
    }

    /// Writes one frame and flushes the transport.
    pub fn write(&self, writer: &mut impl Write) -> io::Result<()> {
        let length = self
            .payload
            .len()
            .checked_add(5)
            .filter(|length| *length <= LIMIT)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "host frame too large"))?;
        writer.write_all(&(length as u32).to_be_bytes())?;
        writer.write_all(&[self.kind])?;
        writer.write_all(&self.channel.to_be_bytes())?;
        writer.write_all(&self.payload)?;
        writer.flush()
    }
}

/// One machine operation, with no feature knowledge on the endpoint.
#[derive(Serialize, Deserialize)]
pub(crate) struct Request {
    /// The operation name.
    pub op: String,
    /// The operation parameters.
    pub args: Value,
}

/// A control result or a portable I/O error.
#[derive(Serialize, Deserialize)]
pub(crate) struct Reply {
    /// The returned control data.
    pub value: Value,
    /// The portable failure, when the operation failed.
    pub error: Option<(String, String)>,
}

/// The host protocol revision, independent of the release version.
pub const PROTOCOL: u64 = 2;

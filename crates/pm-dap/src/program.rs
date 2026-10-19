//! The port an adapter listens on.

use std::io;
use std::net::TcpListener;

/// A port on the loopback address nothing listens on now.
pub(crate) fn free_port() -> io::Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

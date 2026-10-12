//! Connections to services listening on either loopback address family.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Opens the loopback service at `port`, trying IPv6 when IPv4 is unavailable.
pub(crate) fn connect(port: u16) -> io::Result<TcpStream> {
    let timeout = Duration::from_secs(2);
    let ipv4 = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&ipv4, timeout)
        .or_else(|_| TcpStream::connect_timeout(&ipv6, timeout))
}

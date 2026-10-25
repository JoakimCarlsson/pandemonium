//! Finding the address of the developer tools of a page, which the engine serves on a local port.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// Asks the engine, away from the thread that runs it, for the address of the developer tools of the page at `address`, and sends it on the channel returned, or sends nothing found.
///
/// The engine answers from the thread that runs it, so this must not be
/// waited for on that thread.
pub fn find_tools(port: u16, address: String) -> Receiver<Option<String>> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        let found = (0..50).find_map(|_| {
            lookup(port, &address).or_else(|| {
                std::thread::sleep(Duration::from_millis(100));
                None
            })
        });
        let _ = sender.send(found);
    });
    receiver
}

/// The address of the developer tools of the page at `address`, if the engine lists it now.
fn lookup(port: u16, address: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream
        .write_all(
            format!("GET /json HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;
    let mut reader = BufReader::new(stream);
    let mut length = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).ok()?;
        let header = line.trim_end().to_ascii_lowercase();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("content-length:") {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    let targets: serde_json::Value = serde_json::from_slice(&body).ok()?;
    let target = targets
        .as_array()?
        .iter()
        .find(|target| target["type"] == "page" && target["url"].as_str() == Some(address))?;
    let id = target["id"].as_str()?;
    Some(format!(
        "http://127.0.0.1:{port}/devtools/inspector.html?ws=127.0.0.1:{port}/devtools/page/{id}"
    ))
}

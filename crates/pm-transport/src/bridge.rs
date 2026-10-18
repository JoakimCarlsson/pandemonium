//! Carrying the connections of a local port through the standard streams of a program.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Command, Stdio};

use crate::provision::quiet;

/// What a transport says to its caller about how far it has got, one line at a time.
pub type Progress<'a> = &'a dyn Fn(&str);

/// How much is moved in one read.
const CHUNK: usize = 16 * 1024;

/// Carries one connection through `command`, whose standard input and output reach the server, until either end closes.
fn carry(connection: TcpStream, mut command: Command) {
    let Ok(mut child) = quiet(&mut command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let (Some(mut to_remote), Some(mut from_remote)) = (child.stdin.take(), child.stdout.take())
    else {
        return;
    };
    let Ok(mut read_side) = connection.try_clone() else {
        return;
    };
    let mut write_side = connection;
    let upward = std::thread::spawn(move || {
        let mut buffer = [0u8; CHUNK];
        while let Ok(count) = read_side.read(&mut buffer) {
            if count == 0
                || to_remote.write_all(&buffer[..count]).is_err()
                || to_remote.flush().is_err()
            {
                break;
            }
        }
    });
    let mut buffer = [0u8; CHUNK];
    while let Ok(count) = from_remote.read(&mut buffer) {
        if count == 0 || write_side.write_all(&buffer[..count]).is_err() {
            break;
        }
    }
    let _ = write_side.shutdown(Shutdown::Both);
    let _ = child.kill();
    let _ = upward.join();
    let _ = child.wait();
}

/// Opens a local port that carries each connection made to it through a new program from `command`, and returns its address as `host:port`.
pub fn bridge(command: impl Fn() -> Command + Send + Sync + 'static) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let local = listener.local_addr().map_err(|error| error.to_string())?;
    let command = std::sync::Arc::new(command);
    std::thread::spawn(move || {
        for connection in listener.incoming().flatten() {
            let command = command.clone();
            std::thread::spawn(move || carry(connection, command()));
        }
    });
    Ok(local.to_string())
}

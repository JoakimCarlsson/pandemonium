//! Finding the web page that a program started in a shell serves: the address it prints, the port it listens on and what the port answers.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::Duration;

/// How long a probe waits to connect to a port and for what it answers.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

/// How many bytes of an answer are read.
const PROBE_BYTES: usize = 8 * 1024;

/// How many bytes of the page are looked at to tell whether it is a document.
const SNIFF_BYTES: usize = 1024;

/// The program that listens on `port`: its process number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Process(pub(crate) u32);

/// How many bytes of a line that has not ended are kept for the next output.
const PENDING: usize = 4 * 1024;

/// The ports whose page was announced and that still listen, so that a page is announced once however it was found.
static ANNOUNCED: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// Takes the right to announce the page on `port`; false when it was announced and the port has listened ever since.
pub(crate) fn claim(port: u16) -> bool {
    ANNOUNCED.lock().is_ok_and(|mut held| held.insert(port))
}

/// Forgets the announced ports that are not in `listening`, so a program started again on one is announced again.
pub(crate) fn release_unlisted(listening: &BTreeSet<u16>) {
    if let Ok(mut held) = ANNOUNCED.lock() {
        held.retain(|port| listening.contains(port));
    }
}

/// A web page a shell's program says it serves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Found {
    /// The address, which is the page at the root of a local port.
    pub(crate) url: String,
    /// The port.
    pub(crate) port: u16,
}

/// What a port answered at a path.
struct Answer {
    /// The status of the answer.
    status: u16,
    /// The media type it said the body has, in lower case, when it said.
    media_type: Option<String>,
    /// The first bytes of the body, in lower case.
    body: String,
}

/// Takes the text out of what a shell printed: the escape sequences that colour and move the cursor go, and the lines are told apart by line feeds only.
pub(crate) fn plain_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    for next in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    plain.push('\n');
                }
            }
            c => plain.push(c),
        }
    }
    plain
}

/// The page `line` says is served, when it holds a bare address of a local port, such as `Local: http://localhost:5173/`; an address with a path, a query, a login or another machine is not one.
pub(crate) fn found_in(line: &str) -> Option<Found> {
    let start = line.find("http://")?;
    let word = line[start..]
        .split(char::is_whitespace)
        .next()?
        .trim_end_matches([')', ']', ',', '.', ';', '\'', '"']);
    let rest = word.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(cut) => (&rest[..cut], &rest[cut..]),
        None => (rest, ""),
    };
    if !(path.is_empty() || path == "/") || authority.contains('@') {
        return None;
    }
    let (host, port) = authority.rsplit_once(':')?;
    let port: u16 = port.parse().ok().filter(|port| *port != 0)?;
    if !matches!(
        host,
        "localhost" | "127.0.0.1" | "0.0.0.0" | "[::1]" | "[::]"
    ) {
        return None;
    }
    Some(Found {
        url: format!("http://localhost:{port}/"),
        port,
    })
}

/// Adds `data`, what a shell printed, to the text `pending` that has not ended its line yet, and returns the page the last line that ended says is served, if any.
pub(crate) fn scan(pending: &mut String, data: &[u8]) -> Option<Found> {
    pending.push_str(&plain_text(data));
    let found = pending.rfind('\n').and_then(|end| {
        let found = pending[..end].lines().rev().find_map(found_in);
        pending.drain(..=end);
        found
    });
    if pending.len() > PENDING {
        let mut cut = pending.len() - PENDING;
        while !pending.is_char_boundary(cut) {
            cut += 1;
        }
        pending.drain(..cut);
    }
    found
}

/// The ports something on this machine listens on, or nothing when the system cannot say.
pub(crate) fn listening_ports() -> Option<BTreeSet<u16>> {
    if cfg!(target_os = "linux") {
        let mut ports = BTreeSet::new();
        let mut read = false;
        for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
            if let Ok(text) = std::fs::read_to_string(table) {
                read = true;
                ports.extend(proc_ports(&text));
            }
        }
        return read.then_some(ports);
    }
    let output = std::process::Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fn"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    Some(lsof_ports(&String::from_utf8_lossy(&output.stdout)))
}

/// The processes that listen on `port`, which are looked for only when a port is new.
pub(crate) fn listeners_of(port: u16) -> Vec<Process> {
    if cfg!(target_os = "linux") {
        let mut inodes = BTreeSet::new();
        for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
            if let Ok(text) = std::fs::read_to_string(table) {
                inodes.extend(proc_inodes(&text, port));
            }
        }
        if inodes.is_empty() {
            return Vec::new();
        }
        let Ok(processes) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        return processes
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| holds_socket(*pid, &inodes))
            .map(Process)
            .collect();
    }
    command_text(
        "lsof",
        &["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"],
    )
    .lines()
    .filter_map(|line| line.trim().parse().ok())
    .map(Process)
    .collect()
}

/// The inodes of the sockets a table of `/proc/net/tcp` lists as listening on `port`.
fn proc_inodes(table: &str, port: u16) -> Vec<u64> {
    table
        .lines()
        .skip(1)
        .filter_map(|row| {
            let columns: Vec<&str> = row.split_whitespace().collect();
            if *columns.get(3)? != "0A" {
                return None;
            }
            let local = u16::from_str_radix(columns.get(1)?.rsplit_once(':')?.1, 16).ok()?;
            (local == port).then(|| columns.get(9)?.parse().ok())?
        })
        .collect()
}

/// Whether the process `pid` holds one of the sockets `inodes`.
fn holds_socket(pid: u32, inodes: &BTreeSet<u64>) -> bool {
    std::fs::read_dir(format!("/proc/{pid}/fd")).is_ok_and(|descriptors| {
        descriptors.flatten().any(|descriptor| {
            std::fs::read_link(descriptor.path())
                .ok()
                .and_then(|target| {
                    let text = target.to_string_lossy().into_owned();
                    text.strip_prefix("socket:[")?
                        .strip_suffix(']')?
                        .parse::<u64>()
                        .ok()
                })
                .is_some_and(|inode| inodes.contains(&inode))
        })
    })
}

/// What the program `command` with `arguments` prints, or nothing when it cannot be run.
fn command_text(command: &str, arguments: &[&str]) -> String {
    std::process::Command::new(command)
        .args(arguments)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// The folder `process` works in.
pub(crate) fn directory_of(process: Process) -> Option<std::path::PathBuf> {
    if cfg!(target_os = "linux") {
        return std::fs::read_link(format!("/proc/{}/cwd", process.0)).ok();
    }
    command_text(
        "lsof",
        &["-a", "-p", &process.0.to_string(), "-d", "cwd", "-Fn"],
    )
    .lines()
    .find_map(|line| line.strip_prefix('n').map(std::path::PathBuf::from))
}

/// The ports a table of `/proc/net/tcp` lists as listening.
fn proc_ports(table: &str) -> BTreeSet<u16> {
    table
        .lines()
        .skip(1)
        .filter_map(|row| {
            let mut columns = row.split_whitespace().skip(1);
            let local = columns.next()?;
            let state = columns.nth(1)?;
            if state != "0A" {
                return None;
            }
            u16::from_str_radix(local.rsplit_once(':')?.1, 16).ok()
        })
        .collect()
}

/// The ports `lsof -F n` lists as listening.
fn lsof_ports(text: &str) -> BTreeSet<u16> {
    text.lines()
        .filter_map(|line| line.strip_prefix('n')?.rsplit_once(':')?.1.parse().ok())
        .collect()
}

/// What `port` on this machine answers to a request for `path`, or nothing when it does not answer in time.
fn fetch(port: u16, path: &str) -> Option<Answer> {
    let stream = ("localhost", port)
        .to_socket_addrs()
        .ok()?
        .find_map(|address| TcpStream::connect_timeout(&address, PROBE_TIMEOUT).ok())?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT)).ok()?;
    let mut stream = stream;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\nAccept: text/html\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 2048];
    while bytes.len() < PROBE_BYTES {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(count) => bytes.extend_from_slice(&chunk[..count]),
        }
    }
    let text = String::from_utf8_lossy(&bytes);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let mut lines = head.lines();
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let media_type = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-type").then(|| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
    });
    let end = (0..=SNIFF_BYTES.min(body.len()))
        .rev()
        .find(|end| body.is_char_boundary(*end))
        .unwrap_or(0);
    Some(Answer {
        status,
        media_type,
        body: body[..end].to_ascii_lowercase(),
    })
}

/// Whether the root of `port` is a web page: it answers, does not send the reader elsewhere, calls its body a document or does not say, and its body is one, and it is not the discovery endpoint of a browser's debugger.
pub(crate) fn serves_page(port: u16) -> bool {
    let Some(root) = fetch(port, "/") else {
        return false;
    };
    let page = root.status != 404
        && !(300..400).contains(&root.status)
        && root
            .media_type
            .as_deref()
            .is_none_or(|kind| kind == "text/html" || kind == "application/xhtml+xml")
        && (root.body.contains("<!doctype html") || root.body.contains("<html"));
    page && !fetch(port, "/json/version")
        .is_some_and(|answer| answer.status == 200 && answer.body.contains("websocketdebuggerurl"))
}

#[cfg(test)]
mod tests;

//! Tests of the shells the server runs.

use std::collections::{BTreeMap, HashSet};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pm_protocol::ServerMessage;

use super::*;

/// A web server started in a shell is announced to the connection watching the shell, once it listens and answers with a page.
#[test]
fn announces_a_page_a_shell_serves() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let core = Core::new();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/serving-test");
    std::fs::create_dir_all(&directory).expect("folder");
    std::fs::write(directory.join("index.html"), "<!doctype html><html></html>").expect("page");
    let number = core
        .open_shell(&directory, "here", 80, 24, &BTreeMap::new(), None)
        .expect("shell");
    let (outbox, inbox) = channel();
    let connection = Arc::new(Connection {
        outbox,
        attached: Mutex::new(HashSet::new()),
    });
    assert!(core.attach_terminal(&connection, number, 0));
    std::thread::sleep(Duration::from_millis(500));
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("port")
        .port();
    let command = format!(
        "python3 -m http.server {port} --bind 127.0.0.1 >/dev/null 2>&1 & sleep 1; echo '  Local: http://localhost:{port}/'\r"
    );
    assert!(core.type_into_terminal(number, command.as_bytes()));
    let found = std::iter::repeat_with(|| inbox.recv_timeout(Duration::from_secs(15)))
        .map_while(Result::ok)
        .find_map(|message| match message {
            ServerMessage::TerminalServing { terminal, url } => Some((terminal, url)),
            _ => None,
        });
    core.close_terminal(number);
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(found, Some((number, format!("http://localhost:{port}/"))));
}

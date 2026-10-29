//! Tests of finding the pages a session's programs serve.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::*;

/// A web server started in `folder`, as an agent's own tools would start one, and its port.
fn server_in(folder: &Path) -> (Child, u16) {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("port")
        .port();
    let child = Command::new("python3")
        .args([
            "-m",
            "http.server",
            &port.to_string(),
            "--bind",
            "127.0.0.1",
        ])
        .current_dir(folder)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("server");
    for _ in 0..100 {
        if serving::listening_ports().is_some_and(|ports| ports.contains(&port)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    (child, port)
}

/// A listening port is matched to the session whose folder its program works in, and to no other.
#[test]
fn matches_a_port_to_the_session_it_works_for() {
    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/session-servers-test");
    let (mine, other) = (base.join("mine"), base.join("other"));
    std::fs::create_dir_all(&mine).expect("folder");
    std::fs::create_dir_all(&other).expect("folder");
    std::fs::write(mine.join("index.html"), "<!doctype html><html></html>").expect("page");
    let (mut child, port) = server_in(&mine.canonicalize().expect("folder"));
    let folders = [
        (1, other.canonicalize().expect("folder")),
        (2, mine.canonicalize().expect("folder")),
    ];
    let owner = owner_of(port, &folders);
    let alone = owner_of(port, &folders[..1]);
    let served = serving::serves_page(port);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(owner, Some(2));
    assert_eq!(alone, None);
    assert!(served);
}

/// A page is announced once however many times it is found, and again after its port stopped listening.
#[test]
fn announces_a_port_once_until_it_stops_listening() {
    let port = 61_234;
    assert!(serving::claim(port));
    assert!(!serving::claim(port));
    serving::release_unlisted(&BTreeSet::from([port]));
    assert!(!serving::claim(port));
    serving::release_unlisted(&BTreeSet::new());
    assert!(serving::claim(port));
    serving::release_unlisted(&BTreeSet::new());
}

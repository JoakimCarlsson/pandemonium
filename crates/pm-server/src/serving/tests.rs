//! Tests of finding what a shell's program serves.

use super::*;

/// The address of a local port is found through colour codes, and other addresses are not.
#[test]
fn finds_bare_local_addresses() {
    let vite = "  \u{1b}[32m➜\u{1b}[39m  \u{1b}[1mLocal\u{1b}[22m:   \u{1b}[36mhttp://localhost:\u{1b}[1m5173\u{1b}[22m/\u{1b}[39m\n";
    let found = found_in(&plain_text(vite.as_bytes()));
    assert_eq!(
        found,
        Some(Found {
            url: "http://localhost:5173/".to_owned(),
            port: 5173
        })
    );
    assert_eq!(
        found_in("- Local:        http://0.0.0.0:3000").map(|found| found.port),
        Some(3000)
    );
    assert_eq!(
        found_in("Listening on (http://127.0.0.1:8080).").map(|found| found.url),
        Some("http://localhost:8080/".to_owned())
    );
    assert_eq!(found_in("Network: http://192.168.1.4:5173/"), None);
    assert_eq!(found_in("see http://localhost:3000/docs/api"), None);
    assert_eq!(found_in("http://user:pw@localhost:3000/"), None);
    assert_eq!(found_in("http://localhost/"), None);
}

/// A line that ends in a later chunk is found once it ends, and only the last address of a chunk counts.
#[test]
fn scans_across_chunks() {
    let mut pending = String::new();
    assert_eq!(scan(&mut pending, b"  Local: http://local"), None);
    assert_eq!(
        scan(&mut pending, b"host:4000/\r\n").map(|found| found.port),
        Some(4000)
    );
    assert_eq!(
        scan(
            &mut pending,
            b"http://localhost:1111/\nhttp://localhost:2222/\n"
        )
        .map(|found| found.port),
        Some(2222)
    );
}

/// The ports of the system's tables are read.
#[test]
fn reads_listening_ports() {
    let table = "  sl  local_address rem_address   st tx_queue rx_queue\n   0: 0100007F:1435 00000000:0000 0A 00000000:00000000 00:00000000 0 0 1 1\n   1: 0100007F:C350 0100007F:1435 01 00000000:00000000 00:00000000 0 0 2 1\n";
    assert_eq!(proc_ports(table), BTreeSet::from([5173]));
    assert_eq!(
        lsof_ports("p12\nn*:3000\nn127.0.0.1:5173\nn[::1]:8080\n"),
        BTreeSet::from([3000, 5173, 8080])
    );
}

/// Answers a request on a local port with `reply` and returns the port.
fn serve(reply: &'static str) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(2).flatten() {
            let mut stream = stream;
            let mut request = [0u8; 512];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    port
}

/// A page of HTML is told from an API, a redirect and a missing page.
#[test]
fn tells_pages_from_other_servers() {
    let page = serve(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!DOCTYPE html><html></html>",
    );
    assert!(serves_page(page));
    let api = serve(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"ok\":true}",
    );
    assert!(!serves_page(api));
    let missing = serve(
        "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html></html>",
    );
    assert!(!serves_page(missing));
    let moved =
        serve("HTTP/1.1 302 Found\r\nLocation: /login\r\nConnection: close\r\n\r\n<html></html>");
    assert!(!serves_page(moved));
}

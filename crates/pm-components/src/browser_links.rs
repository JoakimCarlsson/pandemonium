//! Where a web link goes: a link to a page on this machine opens in the embedded browser, every other link in the reader's browser.

/// Where a web link opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Route {
    /// The browser pane, on this address.
    Pane(String),
    /// The reader's own browser, on the address as it was given.
    System(String),
}

/// The address to show in the browser pane when `address` is an `http` or `https` address of `localhost`, `127.0.0.1`, `[::1]` or `0.0.0.0`, on any port, and none otherwise.
/// `0.0.0.0` is written as `127.0.0.1`, since it names no place a page can be fetched from.
pub fn local_address(address: &str) -> Option<String> {
    if address
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }
    let (scheme, rest) = address.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(inner) => {
            let (host, after) = inner.split_once(']')?;
            let port = match after {
                "" => None,
                after => Some(after.strip_prefix(':')?),
            };
            (format!("[{host}]"), port)
        }
        None => match authority.split_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port)),
            None => (authority.to_owned(), None),
        },
    };
    if let Some(port) = port
        && (port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u16>().is_err())
    {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let host = match host.as_str() {
        "localhost" | "127.0.0.1" | "[::1]" => host,
        "0.0.0.0" => "127.0.0.1".to_owned(),
        _ => return None,
    };
    let port = port.map_or_else(String::new, |port| format!(":{port}"));
    Some(format!(
        "{}://{host}{port}{tail}",
        scheme.to_ascii_lowercase()
    ))
}

/// Where `address` opens: in the browser pane when it is a local address, `local_links` is on and the embedded browser `ready` (on and installed), in the reader's browser otherwise.
pub fn route(address: &str, local_links: bool, ready: bool) -> Route {
    match local_address(address) {
        Some(local) if local_links && ready => Route::Pane(local),
        _ => Route::System(address.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(address: &str) -> Route {
        route(address, true, true)
    }

    #[test]
    fn local_addresses_go_to_the_pane() {
        for address in [
            "http://localhost",
            "http://localhost:5173/docs?a=1#top",
            "https://localhost:8443/",
            "http://127.0.0.1:3000",
            "http://[::1]:8080/x",
            "http://[::1]/",
            "HTTP://LocalHost:80/",
            "http://localhost?x=1",
        ] {
            assert!(matches!(pane(address), Route::Pane(_)), "{address}");
        }
    }

    #[test]
    fn unspecified_address_is_rewritten() {
        assert_eq!(
            pane("http://0.0.0.0:4000/a"),
            Route::Pane("http://127.0.0.1:4000/a".to_owned())
        );
    }

    #[test]
    fn everything_else_goes_to_the_system_browser() {
        for address in [
            "https://example.com",
            "http://localhost.evil.com/",
            "http://localhost@evil.com/",
            "http://evil.com#@localhost/",
            "http://evil.com/?http://localhost/",
            "http://user:pw@localhost/",
            "http://localhost:99999/",
            "http://localhost:/",
            "http://localhost:80x/",
            "http://localhost:+80/",
            "http://127.0.0.2/",
            "http://127.0.0.1.nip.io/",
            "http://[::2]/",
            "http://[::1]x/",
            "http://0.0.0.1/",
            "http://localhost\\@evil.com/",
            "http://local host/",
            "ftp://localhost/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ssh://localhost",
            "localhost:3000",
            "http://",
            "",
        ] {
            assert_eq!(
                pane(address),
                Route::System(address.to_owned()),
                "{address}"
            );
        }
    }

    #[test]
    fn setting_or_missing_browser_keeps_links_out() {
        let address = "http://localhost:3000";
        let system = Route::System(address.to_owned());
        assert_eq!(route(address, false, true), system);
        assert_eq!(route(address, true, false), system);
        assert_eq!(route(address, false, false), system);
        assert!(matches!(route(address, true, true), Route::Pane(_)));
    }
}

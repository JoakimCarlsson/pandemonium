//! What the address bar makes of what was typed: an address to load, or, when it is not an address, a web search.

/// Where text that is not an address is searched for; the query is added after it. DuckDuckGo is fixed here, since choosing the engine is not a setting yet.
pub const SEARCH_URL: &str = "https://duckduckgo.com/?q=";

/// The address `typed` names: a web address as it is, a bare host with a scheme in front, a local host over plain HTTP, and for anything that is not an address, a web search for it.
pub fn address_of(typed: &str) -> String {
    let typed = typed.trim();
    if typed.is_empty() {
        return "about:blank".to_owned();
    }
    if typed.contains("://") || typed.starts_with("about:") || typed.starts_with("data:") {
        return typed.to_owned();
    }
    if let Some(port) = typed.strip_prefix(':')
        && port.starts_with(|digit: char| digit.is_ascii_digit())
    {
        return format!("http://localhost:{port}");
    }
    if typed.contains(char::is_whitespace) {
        return search_for(typed);
    }
    let host = authority(typed);
    if is_local(host) {
        return format!("http://{typed}");
    }
    if is_host(host) {
        return format!("https://{typed}");
    }
    search_for(typed)
}

/// The part of `typed` before its path, query or fragment: the host, with a port or credentials when written.
fn authority(typed: &str) -> &str {
    typed
        .find(['/', '?', '#'])
        .map_or(typed, |end| &typed[..end])
}

/// `authority` without credentials before it and a port after it.
fn name_of(authority: &str) -> &str {
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.starts_with('[') {
        return authority;
    }
    authority.split(':').next().unwrap_or(authority)
}

/// Whether `authority` names the machine itself: `localhost`, a loopback or unspecified address, or an IPv6 literal.
fn is_local(authority: &str) -> bool {
    let name = name_of(authority);
    name == "localhost"
        || name.ends_with(".localhost")
        || name.starts_with("127.")
        || name == "0.0.0.0"
        || name.starts_with('[')
}

/// Whether `authority` reads as a host to open: it has a dot, as in `example.com` or `10.0.0.5`, or it is a name with a port, as in `devbox:8080`.
fn is_host(authority: &str) -> bool {
    let name = name_of(authority);
    if name.is_empty() {
        return false;
    }
    let with_port = authority.rsplit_once(':').is_some_and(|(_, port)| {
        !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
    });
    name.contains('.') || with_port
}

/// The address of a web search for `query`.
fn search_for(query: &str) -> String {
    let mut address = String::from(SEARCH_URL);
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                address.push(char::from(byte));
            }
            b' ' => address.push('+'),
            other => address.push_str(&format!("%{other:02X}")),
        }
    }
    address
}

#[cfg(test)]
mod tests;

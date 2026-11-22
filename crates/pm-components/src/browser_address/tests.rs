//! Tests of what the address bar makes of what was typed.

use super::{SEARCH_URL, address_of, address_with};

/// Addresses with a scheme are kept as typed.
#[test]
fn addresses_with_a_scheme_stay() {
    assert_eq!(
        address_of("https://example.com/a b"),
        "https://example.com/a b"
    );
    assert_eq!(address_of("http://localhost:5173"), "http://localhost:5173");
    assert_eq!(address_of("about:blank"), "about:blank");
    assert_eq!(address_of("  about:blank "), "about:blank");
}

/// A host gets a scheme: plain HTTP for the local machine, HTTPS for the rest.
#[test]
fn hosts_get_a_scheme() {
    assert_eq!(address_of("example.com"), "https://example.com");
    assert_eq!(address_of("example.com/a?b=c"), "https://example.com/a?b=c");
    assert_eq!(address_of("10.0.0.5:8080/x"), "http://10.0.0.5:8080/x");
    assert_eq!(address_of("devbox:8080"), "http://devbox:8080");
    assert_eq!(address_of("localhost"), "http://localhost");
    assert_eq!(address_of("localhost:3000/a"), "http://localhost:3000/a");
    assert_eq!(address_of("127.0.0.1:9"), "http://127.0.0.1:9");
    assert_eq!(address_of("[::1]:80"), "http://[::1]:80");
    assert_eq!(address_of(":3000"), "http://localhost:3000");
    assert_eq!(
        address_of("app.localhost:3000"),
        "http://app.localhost:3000"
    );
}

/// Text that is not an address is searched for: several words, or one without a dot that is not a local host.
#[test]
fn other_text_is_searched_for() {
    assert_eq!(
        address_of("rust borrow checker"),
        format!("{SEARCH_URL}rust+borrow+checker")
    );
    assert_eq!(address_of("react"), format!("{SEARCH_URL}react"));
    assert_eq!(
        address_of("what is 1.5?"),
        format!("{SEARCH_URL}what+is+1.5%3F")
    );
    assert_eq!(address_of("a&b=c d"), format!("{SEARCH_URL}a%26b%3Dc+d"));
    assert_eq!(
        address_of("kaffe åäö"),
        format!("{SEARCH_URL}kaffe+%C3%A5%C3%A4%C3%B6")
    );
    assert_eq!(address_of("foo:bar"), format!("{SEARCH_URL}foo%3Abar"));
}

/// Nothing typed loads the blank page, not a search for nothing.
#[test]
fn nothing_typed_is_blank() {
    assert_eq!(address_of("   "), "about:blank");
}

/// Names of the local network and single words that read as hosts open over plain HTTP; other single words are searched for at the engine chosen.
#[test]
fn network_hosts_and_known_words_open() {
    assert_eq!(address_of("printer.local"), "http://printer.local");
    assert_eq!(address_of("nas.lan/admin"), "http://nas.lan/admin");
    assert_eq!(address_of("192.168.1.4"), "http://192.168.1.4");
    assert_eq!(address_of("devbox/"), "http://devbox/");
    assert_eq!(address_of("devbox/app"), "http://devbox/app");
    let known = vec!["devbox".to_owned()];
    let engine = "https://www.google.com/search?q=";
    assert_eq!(address_with("devbox", engine, &known), "http://devbox");
    assert_eq!(
        address_with("weather", engine, &known),
        format!("{engine}weather")
    );
    assert_eq!(address_of("1234"), format!("{SEARCH_URL}1234"));
}

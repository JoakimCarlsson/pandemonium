//! Tests of when a page found by a server replaces the one the browser shows.

use super::browser_free_for;

/// A browser on nothing, an error or a page of the same server is sent to the page; one on another page is not.
#[test]
fn only_a_browser_not_in_use_is_sent() {
    let page = "http://localhost:8766/";
    assert!(browser_free_for("", page));
    assert!(browser_free_for("about:blank", page));
    assert!(browser_free_for("chrome-error://chromewebdata/", page));
    assert!(browser_free_for("http://localhost:8766/", page));
    assert!(browser_free_for(
        "http://localhost:8766/form.html?a=1",
        page
    ));
    assert!(!browser_free_for("http://localhost:8765/", page));
    assert!(!browser_free_for("http://localhost:87660/", page));
    assert!(!browser_free_for("https://example.com/", page));
}

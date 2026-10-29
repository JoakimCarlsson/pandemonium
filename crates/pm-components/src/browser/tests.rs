//! Tests of what a page found by a server does to the page the browser shows.

use super::{Move, served_move};

/// A browser on nothing or an error loads the page, one on a page of the same server loads it again where it is, and one on another page stays.
#[test]
fn a_page_in_use_is_left_alone() {
    let page = "http://localhost:8766/";
    assert_eq!(served_move("", page), Move::Load);
    assert_eq!(served_move("about:blank", page), Move::Load);
    assert_eq!(
        served_move("chrome-error://chromewebdata/", page),
        Move::Load
    );
    assert_eq!(served_move("http://localhost:8766/", page), Move::Reload);
    assert_eq!(
        served_move("http://localhost:8766/form.html?a=1", page),
        Move::Reload
    );
    assert_eq!(served_move("http://localhost:8765/", page), Move::Stay);
    assert_eq!(served_move("http://localhost:87660/", page), Move::Stay);
    assert_eq!(served_move("https://example.com/", page), Move::Stay);
}

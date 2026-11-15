//! What the browser decides about the tools agents call: which addresses an agent may open, which tools change a page and so stay out of a locked tab, and what an agent is told of the tabs.

use crate::browser::Browser;

/// What a tool of an agent that only reads a page, and so is allowed on a locked tab, is called.
const READS: [&str; 5] = [
    "browser_snapshot",
    "browser_screenshot",
    "browser_console",
    "browser_get_bounding_box",
    "browser_wait",
];

/// Whether the tool `tool` may change a page: every tool does except those that only read it, and a tool nobody classified counts as changing it.
pub fn changes_page(tool: &str) -> bool {
    !(READS.contains(&tool) || tool.starts_with("browser_network"))
}

/// What a tool of an agent says when it is kept out of a locked tab.
pub fn locked_message(tool: &str, tab: u32) -> String {
    format!(
        "Tab {tab} is locked by the person using the browser, so {tool} cannot change it. Open a tab of your own with browser_tabs (action \"open\") and work there; reading the locked tab with browser_snapshot, browser_screenshot, browser_console and browser_network is still allowed."
    )
}

/// Whether an agent may send the browser to `url`: a web address or the blank page, never a local file or an internal page of the browser.
pub fn agent_may_open(url: &str) -> bool {
    let url = url.trim();
    let scheme = url
        .split_once(':')
        .map_or("", |(scheme, _)| scheme)
        .to_ascii_lowercase();
    url.eq_ignore_ascii_case("about:blank")
        || (matches!(scheme.as_str(), "http" | "https") && url[scheme.len()..].starts_with("://"))
}

/// The scheme, host and port of a web address, in lower case with the scheme's usual port left out; nothing when it is not a plain `http` or `https` address, or has a login, a backslash or a space in it.
fn origin_parts(address: &str) -> Option<(String, String, String)> {
    let address = address.trim();
    if address
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }
    let (scheme, rest) = address.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(inner) => {
            let (host, after) = inner.split_once(']')?;
            (
                format!("[{host}]"),
                after.strip_prefix(':').unwrap_or("").to_owned(),
            )
        }
        None => {
            let (host, port) = authority.split_once(':').unwrap_or((authority, ""));
            (host.to_owned(), port.to_owned())
        }
    };
    let usual = if scheme == "http" { "80" } else { "443" };
    let port = if port == usual { "" } else { port.as_str() };
    Some((scheme, host.to_ascii_lowercase(), port.to_owned()))
}

/// Whether the restriction on agent browsing lets an agent open `url`: always when it is off or the blank page; otherwise only an address of `localhost`, `127.0.0.1` or `[::1]`, or of an origin in `origins`, as `helpers/electron-browser/src/origins.js` decides for the tools the helper runs.
pub fn origin_allowed(url: &str, restricted: bool, origins: &[String]) -> bool {
    if !restricted || url.trim().eq_ignore_ascii_case("about:blank") {
        return true;
    }
    let Some((scheme, host, port)) = origin_parts(url) else {
        return false;
    };
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") {
        return true;
    }
    origins.iter().any(|line| {
        let line = line.trim();
        let candidates = if line.contains("://") {
            vec![line.to_owned()]
        } else {
            vec![format!("http://{line}"), format!("https://{line}")]
        };
        candidates
            .iter()
            .filter_map(|candidate| origin_parts(candidate))
            .any(|entry| entry == (scheme.clone(), host.clone(), port.clone()))
    })
}

/// What an agent is told of the tabs of `browser`: one line for each, with its number and what marks it.
pub fn tabs_text(browser: Option<&Browser>) -> String {
    let Some(browser) = browser else {
        return "No tabs are open.".to_owned();
    };
    let target = browser.agent_target();
    browser
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let marks: Vec<&str> = [
                (tab.id == target, "the tools' tab"),
                (index == browser.active, "shown to the reader"),
                (tab.agent, "opened by an agent"),
                (tab.locked, "locked by the reader"),
            ]
            .into_iter()
            .filter_map(|(on, mark)| on.then_some(mark))
            .collect();
            let marks = if marks.is_empty() {
                String::new()
            } else {
                format!(" ({})", marks.join(", "))
            };
            format!("Tab {}{marks}: {} - {}", tab.id, tab.label(), tab.address)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The restriction lets local addresses and listed origins through, and nothing else.
    #[test]
    fn restriction_holds_new_tabs_to_local_and_listed_origins() {
        let list = vec![
            "https://docs.rs".to_owned(),
            "staging.example.com:8443".to_owned(),
        ];
        assert!(origin_allowed("https://evil.com", false, &list));
        for ok in [
            "about:blank",
            "http://localhost:3000/x",
            "http://[::1]:80/",
            "https://docs.rs/a",
            "https://staging.example.com:8443/",
            "http://staging.example.com:8443",
        ] {
            assert!(origin_allowed(ok, true, &list), "{ok}");
        }
        for bad in [
            "https://evil.com",
            "http://docs.rs/",
            "https://docs.rs:444/",
            "http://localhost@evil.com/",
            "http://evil.com\\@localhost/",
            "file:///etc/passwd",
            "https://staging.example.com/",
            "http://localhost.evil.com",
        ] {
            assert!(!origin_allowed(bad, true, &list), "{bad}");
        }
    }

    /// Only web addresses and the blank page may be opened by agents, and reading tools are allowed on locked tabs.
    #[test]
    fn agents_open_web_pages_and_read_locked_tabs() {
        assert!(agent_may_open("https://example.com"));
        assert!(agent_may_open("about:blank"));
        assert!(!agent_may_open("file:///etc/passwd"));
        assert!(!agent_may_open("chrome://settings"));
        assert!(!changes_page("browser_screenshot"));
        assert!(!changes_page("browser_network"));
        assert!(changes_page("browser_click"));
    }

    /// The list marks the tools' tab and the one shown.
    #[test]
    fn tabs_are_listed_with_their_marks() {
        let mut browser = Browser::new("http://localhost:1", None);
        let id = browser
            .open_tab("http://localhost:2", None, true, false)
            .unwrap_or_default();
        let text = tabs_text(Some(&browser));
        assert!(text.contains(&format!("Tab {id} (the tools' tab, opened by an agent)")));
        assert!(text.contains("shown to the reader"));
        assert_eq!(tabs_text(None), "No tabs are open.");
    }
}

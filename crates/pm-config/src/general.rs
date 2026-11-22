//! How the window opens, closes and keeps itself up to date.

use serde::{Deserialize, Serialize};

/// What the window shows when it opens.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Launch {
    /// The workspace as it was left: the page, the open files and the panels.
    #[default]
    Restore,
    /// The projects alone, on no session and with no file open.
    Fresh,
}

/// Which releases the window looks for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// The releases made for everyone.
    #[default]
    Stable,
    /// The pre-releases; a setting written as `nightly` before the channel was renamed reads as this.
    #[serde(alias = "nightly")]
    Preview,
}

/// Where the embedded browser's address bar searches for what is not an address.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngine {
    /// DuckDuckGo.
    #[default]
    DuckDuckGo,
    /// Google.
    Google,
    /// Bing.
    Bing,
    /// Brave Search.
    Brave,
    /// Startpage.
    Startpage,
}

impl SearchEngine {
    /// Every engine, in the order the setting offers them.
    pub const ALL: [Self; 5] = [
        Self::DuckDuckGo,
        Self::Google,
        Self::Bing,
        Self::Brave,
        Self::Startpage,
    ];

    /// What the setting calls it.
    pub const fn title(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Google => "Google",
            Self::Bing => "Bing",
            Self::Brave => "Brave",
            Self::Startpage => "Startpage",
        }
    }

    /// The address of a search, to which the query is added, escaped.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "https://duckduckgo.com/?q=",
            Self::Google => "https://www.google.com/search?q=",
            Self::Bing => "https://www.bing.com/search?q=",
            Self::Brave => "https://search.brave.com/search?q=",
            Self::Startpage => "https://www.startpage.com/do/search?q=",
        }
    }
}

/// How the window opens, closes and keeps itself up to date.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// What the window shows when it opens.
    pub launch: Launch,
    /// Whether closing the window while agents work asks first.
    pub confirm_close: bool,
    /// Whether the window looks for a newer release while it is open.
    pub check_updates: bool,
    /// Which releases it looks for.
    pub channel: Channel,
    /// Whether opening a file written in a language that is not installed offers to install it.
    pub offer_languages: bool,
    /// The languages, by folder name, the reader turned the offer down for.
    pub declined_languages: Vec<String>,
    /// Whether the embedded browser is turned on, which downloads it once.
    pub embedded_browser: bool,
    /// Whether the embedded browser opens on the web page a program started in a shell serves.
    pub open_dev_servers: bool,
    /// Whether a link to a page on this machine, such as `http://localhost:3000`, opens in the embedded browser instead of the system browser.
    pub open_local_links: bool,
    /// Whether the tools agents browse with may open only local addresses and the origins listed in `agent_origins`.
    pub restrict_agent_browsing: bool,
    /// The origins agents may open besides local addresses while `restrict_agent_browsing` is on, such as `https://example.com`.
    pub agent_origins: Vec<String>,
    /// Where the embedded browser's address bar searches for what is not an address.
    pub search_engine: SearchEngine,
}

impl Default for GeneralSettings {
    /// The window as it comes: opening where it was left, asking before closing over working agents and looking for stable releases.
    fn default() -> Self {
        Self {
            launch: Launch::Restore,
            confirm_close: true,
            check_updates: true,
            channel: Channel::Stable,
            offer_languages: true,
            declined_languages: Vec::new(),
            embedded_browser: false,
            open_dev_servers: true,
            open_local_links: true,
            restrict_agent_browsing: false,
            agent_origins: Vec::new(),
            search_engine: SearchEngine::DuckDuckGo,
        }
    }
}

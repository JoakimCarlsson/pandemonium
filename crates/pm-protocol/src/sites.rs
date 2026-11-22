//! What the embedded browser remembers of the sites the reader visits: the zoom of each origin and the addresses visited. The server of the reader's own machine keeps it; a window holds a copy so that the address bar suggests as the reader types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How many visited addresses are kept; the ones visited least recently go first.
pub const VISITS_KEPT: usize = 500;

/// The zoom a site is shown at when the reader never zoomed it, in percent.
pub const PLAIN_ZOOM: u32 = 100;

/// One address the reader visited.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Visit {
    /// The address.
    pub url: String,
    /// The title the page had.
    pub title: String,
    /// How many times it was visited.
    pub count: u32,
    /// When it was last visited, in seconds since the Unix epoch.
    pub last: u64,
}

/// What the browser remembers of sites.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserSites {
    /// The zoom of each origin the reader zoomed, in percent; origins at 100% are not kept.
    pub zoom: BTreeMap<String, u32>,
    /// The addresses visited, the most recent last.
    pub visits: Vec<Visit>,
}

/// The scheme, host and port of a web address, in lower case: `https://example.com:8443`; nothing when it is not an `http` or `https` address.
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.to_ascii_lowercase();
    (!host.is_empty()).then(|| format!("{scheme}://{host}"))
}

/// The host of a web address without its port, in lower case.
pub fn site_host_of(url: &str) -> Option<String> {
    let origin = origin_of(url)?;
    let authority = origin.split_once("://")?.1;
    if authority.starts_with('[') {
        return Some(authority.split(']').next()?.to_owned() + "]");
    }
    Some(authority.split(':').next()?.to_owned())
}

/// A zoom factor, such as `1.25`, in whole percent, such as `125`.
pub fn zoom_percent(factor: f32) -> u32 {
    (factor * 100.0).round().max(1.0) as u32
}

impl BrowserSites {
    /// The zoom, in percent, the reader left the origin of `url` at, when it is not 100%.
    pub fn zoom_of(&self, url: &str) -> Option<u32> {
        self.zoom.get(&origin_of(url)?).copied()
    }

    /// Remembers that the reader zoomed the origin of `url` to `percent`; 100% forgets it. Says whether anything changed.
    pub fn set_zoom(&mut self, url: &str, percent: u32) -> bool {
        let Some(origin) = origin_of(url) else {
            return false;
        };
        if percent == PLAIN_ZOOM {
            self.zoom.remove(&origin).is_some()
        } else {
            self.zoom.insert(origin, percent) != Some(percent)
        }
    }

    /// Remembers a visit to `url`, a web address, called `title`, at `at` seconds since the Unix epoch. Says whether anything changed.
    pub fn visited(&mut self, url: &str, title: &str, at: u64) -> bool {
        if origin_of(url).is_none() {
            return false;
        }
        let visit = match self.visits.iter().position(|visit| visit.url == url) {
            Some(index) => {
                let mut visit = self.visits.remove(index);
                visit.count = visit.count.saturating_add(1);
                visit
            }
            None => Visit {
                url: url.to_owned(),
                count: 1,
                ..Visit::default()
            },
        };
        self.visits.push(Visit {
            title: if title.is_empty() {
                visit.title.clone()
            } else {
                title.to_owned()
            },
            last: at,
            ..visit
        });
        if self.visits.len() > VISITS_KEPT {
            let extra = self.visits.len() - VISITS_KEPT;
            self.visits.drain(..extra);
        }
        true
    }

    /// Gives the visit to `url` the title `title`, which the page reported after it loaded. Says whether anything changed.
    pub fn titled(&mut self, url: &str, title: &str) -> bool {
        if title.is_empty() {
            return false;
        }
        match self.visits.iter_mut().find(|visit| visit.url == url) {
            Some(visit) if visit.title != title => {
                visit.title = title.to_owned();
                true
            }
            _ => false,
        }
    }

    /// Up to `limit` visits whose address or title holds what was `typed`, those whose address starts with it, without its scheme, first, then the most visited and most recent.
    pub fn suggestions(&self, typed: &str, limit: usize) -> Vec<Visit> {
        let typed = typed.trim().to_lowercase();
        if typed.is_empty() {
            return Vec::new();
        }
        let bare = |url: &str| {
            let url = url.split_once("://").map_or(url, |(_, rest)| rest);
            url.strip_prefix("www.").unwrap_or(url).to_lowercase()
        };
        let mut found: Vec<(bool, &Visit)> = self
            .visits
            .iter()
            .filter_map(|visit| {
                let address = bare(&visit.url);
                let starts =
                    address.starts_with(&typed) || visit.url.to_lowercase().starts_with(&typed);
                (starts || address.contains(&typed) || visit.title.to_lowercase().contains(&typed))
                    .then_some((starts, visit))
            })
            .collect();
        found.sort_by(|(a_starts, a), (b_starts, b)| {
            b_starts
                .cmp(a_starts)
                .then(b.count.cmp(&a.count))
                .then(b.last.cmp(&a.last))
        });
        found
            .into_iter()
            .take(limit)
            .map(|(_, visit)| visit.clone())
            .collect()
    }

    /// The hosts without a dot the reader visited, such as `devbox`, which the address bar opens rather than searches for.
    pub fn bare_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self
            .visits
            .iter()
            .filter_map(|visit| site_host_of(&visit.url))
            .filter(|host| !host.contains('.') && !host.starts_with('['))
            .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }
}

#[cfg(test)]
/// Tests for what is remembered of sites.
mod tests {
    use super::*;

    /// Origins keep the scheme, host and port, in lower case, and only of web addresses.
    #[test]
    fn origins_are_scheme_host_and_port() {
        assert_eq!(
            origin_of("https://Example.com:8443/a?b#c").as_deref(),
            Some("https://example.com:8443")
        );
        assert_eq!(origin_of("file:///etc/passwd"), None);
        assert_eq!(
            site_host_of("http://devbox:3000/x").as_deref(),
            Some("devbox")
        );
    }

    /// The zoom of an origin is remembered for every page of it, and 100% forgets it.
    #[test]
    fn zoom_is_kept_per_origin() {
        let mut sites = BrowserSites::default();
        assert!(sites.set_zoom("https://example.com/a", 150));
        assert!(!sites.set_zoom("https://example.com/a", 150));
        assert_eq!(sites.zoom_of("https://example.com/b"), Some(150));
        assert_eq!(sites.zoom_of("http://example.com/b"), None);
        assert!(sites.set_zoom("https://example.com/c", PLAIN_ZOOM));
        assert_eq!(sites.zoom_of("https://example.com/a"), None);
        assert_eq!(zoom_percent(1.25), 125);
    }

    /// Suggestions put addresses that start with what was typed first, and bare hosts are known.
    #[test]
    fn suggestions_prefer_addresses_that_start_with_the_text() {
        let mut sites = BrowserSites::default();
        sites.visited("https://docs.rs/serde", "serde - Rust", 1);
        sites.visited("http://devbox:3000/", "Dev", 2);
        sites.visited("https://example.com/devbox", "Example", 3);
        assert!(!sites.visited("file:///etc/passwd", "", 4));
        let found = sites.suggestions("dev", 5);
        assert_eq!(found[0].url, "http://devbox:3000/");
        assert_eq!(found.len(), 2);
        assert_eq!(sites.bare_hosts(), vec!["devbox".to_owned()]);
    }

    /// Visiting an address again counts it, moves it last and keeps its title when none is given.
    #[test]
    fn visiting_again_counts_and_keeps_the_title() {
        let mut sites = BrowserSites::default();
        sites.visited("https://a.example/", "A", 1);
        sites.visited("https://b.example/", "B", 2);
        sites.visited("https://a.example/", "", 3);
        let last = sites.visits.last().unwrap();
        assert_eq!((last.count, last.title.as_str(), last.last), (2, "A", 3));
        assert!(sites.titled("https://a.example/", "A again"));
        assert!(!sites.titled("https://a.example/", "A again"));
    }
}

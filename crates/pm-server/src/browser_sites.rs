//! What the reader's browser remembers of sites, kept by the server of the reader's own machine for every window that connects to it: the zoom of each origin and the addresses visited, in a file in the app's data folder.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pm_protocol::{BrowserSites, Response, ServerMessage, Visit, zoom_percent};
use serde::Deserialize;

use crate::core::{Connection, Core};
use crate::handler::failed;

/// What the server keeps of sites for the browsers of the windows connected to it.
pub(crate) static SITES: Store = Store::new("browser-sites", "browser-sites.json");

/// What is remembered of sites, read from its file the first time it is needed and written back whenever it changes.
pub(crate) struct Store {
    /// The name of its file in the app's data folder.
    name: &'static str,
    /// The name of the file windows kept it in before the server kept it, read once when the server's own file is missing and then removed.
    legacy: &'static str,
    /// What is remembered, once it was read.
    sites: Mutex<Option<BrowserSites>>,
}

/// What windows kept of sites before the server kept it, with each zoom as a factor rather than in percent.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Legacy {
    /// The zoom of each origin, as a factor.
    zoom: BTreeMap<String, f32>,
    /// The addresses visited, the most recent last.
    visits: Vec<Visit>,
}

/// The time now, in seconds since the Unix epoch.
fn seconds_now() -> u64 {
    crate::core::now() / 1000
}

impl Store {
    /// A store kept in the file `name` of the app's data folder, taking over the file `legacy` windows kept before.
    pub(crate) const fn new(name: &'static str, legacy: &'static str) -> Self {
        Self {
            name,
            legacy,
            sites: Mutex::new(None),
        }
    }

    /// Where the file named `name` is kept, or nothing when this machine has no data folder.
    fn path(name: &str) -> Option<PathBuf> {
        pm_store::data_directory().map(|data| data.join(name))
    }

    /// What is remembered in the store's file, or what windows kept in the legacy file, converted and saved in the store's file, which is then removed; nothing remembered when neither can be read.
    fn load(&self) -> BrowserSites {
        if let Some(sites) = pm_store::load_text(self.name)
            .and_then(|text| serde_json::from_str::<BrowserSites>(&text).ok())
        {
            return sites;
        }
        let Some(legacy_path) = Self::path(self.legacy) else {
            return BrowserSites::default();
        };
        let Some(legacy) = std::fs::read(&legacy_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Legacy>(&bytes).ok())
        else {
            return BrowserSites::default();
        };
        let sites = BrowserSites {
            zoom: legacy
                .zoom
                .into_iter()
                .map(|(origin, factor)| (origin, zoom_percent(factor)))
                .collect(),
            visits: legacy.visits,
            ..BrowserSites::default()
        };
        if self.save(&sites).is_ok() {
            let _ = std::fs::remove_file(legacy_path);
        }
        sites
    }

    /// Writes `sites` to the store's file through a file beside it that then takes its place, so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Fails when this machine has no data folder, or the folder or the file cannot be written.
    fn save(&self, sites: &BrowserSites) -> Result<(), String> {
        let path = Self::path(self.name).ok_or("the machine has no data folder")?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        let text = serde_json::to_string(sites).map_err(|error| error.to_string())?;
        let partial = path.with_extension("partial");
        std::fs::write(&partial, text).map_err(|error| error.to_string())?;
        std::fs::rename(&partial, &path).map_err(|error| error.to_string())
    }

    /// Runs `work` on what is remembered, reading it first when it was not yet read.
    fn with<T>(&self, work: impl FnOnce(&mut BrowserSites) -> T) -> T {
        let mut held = self
            .sites
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        work(held.get_or_insert_with(|| self.load()))
    }

    /// What is remembered now.
    pub(crate) fn snapshot(&self) -> BrowserSites {
        self.with(|sites| sites.clone())
    }

    /// Makes `change` to what is remembered, which says whether it changed anything, and writes it when it did; gives what is remembered afterwards when it changed.
    ///
    /// # Errors
    ///
    /// Fails when the change could not be written.
    pub(crate) fn change(
        &self,
        change: impl FnOnce(&mut BrowserSites) -> bool,
    ) -> Result<Option<BrowserSites>, String> {
        self.with(|sites| {
            if !change(sites) {
                return Ok(None);
            }
            self.save(sites)?;
            Ok(Some(sites.clone()))
        })
    }

    /// Records a visit to `url` called `title`, once for each `id`, the client's name for the visit, which is saved with the visit so that it is counted once even across a restart; a visit without one is always recorded.
    ///
    /// # Errors
    ///
    /// Fails when the change could not be written.
    pub(crate) fn visited(
        &self,
        url: &str,
        title: &str,
        id: Option<&str>,
    ) -> Result<Option<BrowserSites>, String> {
        let at = seconds_now();
        self.change(|sites| sites.visited_as(url, title, at, id))
    }
}

/// The answer to a change to what is remembered of sites, telling every other client what is remembered now when it changed.
pub(crate) fn answer(
    core: &Arc<Core>,
    connection: &Arc<Connection>,
    changed: Result<Option<BrowserSites>, String>,
) -> Response {
    match changed {
        Ok(Some(sites)) => {
            core.broadcast(
                &ServerMessage::BrowserSitesChanged { sites },
                Some(connection),
            );
            Response::Done
        }
        Ok(None) => Response::Done,
        Err(reason) => failed(reason),
    }
}

#[cfg(test)]
/// Tests for the server's store of sites.
mod tests {
    use super::*;

    /// A visit and a zoom are written to the store's file and read back by a store that starts afresh.
    #[test]
    fn changes_survive_a_restart() {
        let store = Store::new("browser-sites-restart", "browser-sites-restart.json");
        let changed = store
            .visited("https://docs.rs/serde", "serde", None)
            .unwrap();
        assert_eq!(changed.unwrap().visits.len(), 1);
        store
            .change(|sites| sites.set_zoom("https://docs.rs/x", 125))
            .unwrap();
        assert_eq!(
            store.change(|sites| sites.set_zoom("https://docs.rs/x", 125)),
            Ok(None)
        );
        let again = Store::new("browser-sites-restart", "browser-sites-restart.json");
        let sites = again.snapshot();
        assert_eq!(sites.zoom_of("https://docs.rs/"), Some(125));
        assert_eq!(sites.visits[0].title, "serde");
        assert_eq!(sites.suggestions("docs", 5).len(), 1);
    }

    /// The file windows kept is taken over once, its zoom factors turned into percent, and then removed.
    #[test]
    fn the_windows_file_is_taken_over_once() {
        let legacy = Store::path("browser-sites-legacy.json").unwrap();
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy,
            r#"{"zoom":{"https://example.com":1.5},"visits":[{"url":"http://devbox:3000/","title":"Dev","count":2,"last":7}]}"#,
        )
        .unwrap();
        let store = Store::new("browser-sites-taken", "browser-sites-legacy.json");
        let sites = store.snapshot();
        assert_eq!(sites.zoom_of("https://example.com/a"), Some(150));
        assert_eq!(sites.visits[0].count, 2);
        assert!(!legacy.exists());
        let again = Store::new("browser-sites-taken", "browser-sites-legacy.json");
        assert_eq!(again.snapshot(), sites);
    }

    /// Nothing is remembered when there is no file at all.
    #[test]
    fn nothing_is_remembered_at_first() {
        let store = Store::new("browser-sites-empty", "browser-sites-empty.json");
        assert_eq!(store.snapshot(), BrowserSites::default());
    }

    /// A visit sent again under the same id is counted once, while one without an id is counted each time.
    #[test]
    fn a_visit_sent_again_counts_once() {
        let store = Store::new("browser-sites-again", "browser-sites-again.json");
        assert!(
            store
                .visited("https://a.com/", "A", Some("v1"))
                .unwrap()
                .is_some()
        );
        assert_eq!(store.visited("https://a.com/", "A", Some("v1")), Ok(None));
        assert_eq!(store.snapshot().visits[0].count, 1);
        store.visited("https://a.com/", "A", Some("v2")).unwrap();
        store.visited("https://a.com/", "A", None).unwrap();
        assert_eq!(store.snapshot().visits[0].count, 3);
        let _ = std::fs::remove_file(Store::path("browser-sites-again").unwrap());
    }

    /// A visit sent again under the same id after the server restarted is still counted once.
    #[test]
    fn a_visit_sent_again_after_a_restart_counts_once() {
        let store = Store::new("browser-sites-restarted", "browser-sites-restarted.json");
        store.visited("https://a.com/", "A", Some("v1")).unwrap();
        let again = Store::new("browser-sites-restarted", "browser-sites-restarted.json");
        assert_eq!(again.visited("https://a.com/", "A", Some("v1")), Ok(None));
        assert_eq!(again.snapshot().visits[0].count, 1);
        let _ = std::fs::remove_file(Store::path("browser-sites-restarted").unwrap());
    }
}

//! Pictures of documents an agent wrote, rendered by the helper sealed from the network: how large a document may be, the viewport it is rendered at, and the file its picture is kept in, named after what is rendered so that the same document is rendered once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use sha2::{Digest, Sha256};

/// The most bytes of HTML that are rendered, as the harness allows an agent to write.
pub const CONTENT_LIMIT: usize = 256 * 1024;

/// The viewport a document is rendered at, in CSS pixels: its width and height.
pub const VIEWPORT: (u32, u32) = (1024, 640);

/// How long the helper is given to answer a render, or to paint a sealed page it opened, before the window gives up on it: longer than the helper's own limit on a render, so that the helper's answer comes first when it answers at all.
pub const ANSWER_WITHIN: Duration = Duration::from_secs(15);

/// What a render the helper never answered says in place of its picture.
pub const NO_ANSWER: &str = "The browser did not render the preview within 15 seconds.";

/// How long a picture is kept after it was last written before [`prune`] removes it.
const KEPT: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// How a render the helper was asked for ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rendered {
    /// The caller's number for the render.
    pub ticket: u64,
    /// Where the picture is, or why there is none.
    pub outcome: Result<PathBuf, String>,
}

/// What waits for an answer from the helper, by the caller's number, each with the time it was asked at, so that what is never answered is given up on after [`ANSWER_WITHIN`].
#[derive(Debug)]
pub struct Waiting<T> {
    /// What waits, with when it was asked for.
    held: HashMap<u64, (T, Instant)>,
}

impl<T> Default for Waiting<T> {
    /// Nothing waiting.
    fn default() -> Self {
        Self {
            held: HashMap::new(),
        }
    }
}

impl<T> Waiting<T> {
    /// Records `value` as waiting under `ticket` since `asked`.
    pub fn insert(&mut self, ticket: u64, value: T, asked: Instant) {
        self.held.insert(ticket, (value, asked));
    }

    /// Takes what waited under `ticket`, now that it was answered.
    pub fn answered(&mut self, ticket: u64) -> Option<T> {
        self.held.remove(&ticket).map(|(value, _)| value)
    }

    /// Takes everything that has waited longer than [`ANSWER_WITHIN`] by `now`.
    pub fn overdue(&mut self, now: Instant) -> Vec<T> {
        let late: Vec<u64> = self
            .held
            .iter()
            .filter(|(_, (_, asked))| now.saturating_duration_since(*asked) > ANSWER_WITHIN)
            .map(|(ticket, _)| *ticket)
            .collect();
        late.into_iter()
            .filter_map(|ticket| self.answered(ticket))
            .collect()
    }

    /// Takes everything that waits, as when the helper stopped.
    pub fn drain(&mut self) -> Vec<T> {
        self.held.drain().map(|(_, (value, _))| value).collect()
    }

    /// Whether nothing waits.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

/// Why `html` cannot be rendered, if it cannot: it is empty, or larger than [`CONTENT_LIMIT`].
pub fn refusal(html: &str) -> Option<String> {
    if html.is_empty() {
        return Some("There is no document to show.".to_owned());
    }
    (html.len() > CONTENT_LIMIT).then(|| "The document is larger than 256 KiB.".to_owned())
}

/// The display scale a document is rendered at for a window at `scale`: kept between 1 and 4, as the helper keeps it, and 1 for a scale that is not a number.
pub fn render_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(1.0, 4.0)
    } else {
        1.0
    }
}

/// The name of the file the picture of `html` at a viewport of `width` by `height` and display `scale` is kept in: the SHA-256 of the viewport, the scale and the document, so that a document rendered before is found again and a changed one is not. Pictures at scale 1 keep the names they had before the scale was part of them.
pub fn cache_name(html: &str, width: u32, height: u32, scale: f32) -> String {
    let hundredths = (render_scale(scale) * 100.0).round() as u32;
    let viewport = if hundredths == 100 {
        format!("{width}x{height}")
    } else {
        format!("{width}x{height}@{hundredths}")
    };
    let mut hash = Sha256::new();
    hash.update(format!("sealed-render-1\n{viewport}\n").as_bytes());
    hash.update(html.as_bytes());
    let digest: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{digest}.png")
}

/// The folder the pictures are kept in, under the user's cache folder, or nothing when this machine has none.
pub fn cache_folder() -> Option<PathBuf> {
    pm_store::cache_directory().map(|cache| cache.join("sealed-renders"))
}

/// The file the picture of `html` at a viewport of `width` by `height` and display `scale` is kept in, whether or not it is there yet; nothing when this machine has no cache folder.
pub fn cache_file(html: &str, width: u32, height: u32, scale: f32) -> Option<PathBuf> {
    cache_folder().map(|folder| folder.join(cache_name(html, width, height, scale)))
}

/// Removes the pictures in `folder` that were written longer ago than they are kept, so the folder does not grow without end; what cannot be read or removed is left.
pub fn prune(folder: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|written| now.duration_since(written).ok())
            .is_some_and(|age| age > KEPT);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
/// Tests for how rendered pictures are keyed and what is refused.
mod tests {
    use super::*;

    /// The same document at the same viewport has the same file, and a change to either gives another.
    #[test]
    fn the_file_is_named_after_the_document_and_the_viewport() {
        let name = cache_name("<p>a</p>", 1024, 640, 1.0);
        assert_eq!(name, cache_name("<p>a</p>", 1024, 640, 1.0));
        assert_eq!(name, cache_name("<p>a</p>", 1024, 640, 0.5));
        assert_ne!(name, cache_name("<p>b</p>", 1024, 640, 1.0));
        assert_ne!(name, cache_name("<p>a</p>", 640, 1024, 1.0));
        assert_ne!(name, cache_name("<p>a</p>", 1024, 641, 1.0));
        assert_ne!(name, cache_name("<p>a</p>", 1024, 640, 2.0));
        assert!(name.ends_with(".png"));
        assert_eq!(name.len(), 64 + 4);
        assert!(name[..64].bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    /// What is answered is taken once; what is not answered is given up on after the time allowed and not before; what is left goes when everything is drained.
    #[test]
    fn renders_that_are_never_answered_are_given_up_on() {
        let start = Instant::now();
        let mut waiting = Waiting::default();
        waiting.insert(1, "answered", start);
        waiting.insert(2, "silent", start);
        waiting.insert(3, "later", start + Duration::from_secs(10));
        assert_eq!(waiting.answered(1), Some("answered"));
        assert_eq!(waiting.answered(1), None);
        assert!(waiting.overdue(start + ANSWER_WITHIN).is_empty());
        let late = start + ANSWER_WITHIN + Duration::from_secs(1);
        assert_eq!(waiting.overdue(late), vec!["silent"]);
        assert!(waiting.overdue(late).is_empty());
        assert!(!waiting.is_empty());
        assert_eq!(waiting.drain(), vec!["later"]);
        assert!(waiting.is_empty());
    }

    /// An empty document and one over the cap are refused; one at the cap is not.
    #[test]
    fn empty_and_oversized_documents_are_refused() {
        assert!(refusal("").is_some());
        assert!(refusal("<p>hi</p>").is_none());
        assert!(refusal(&"x".repeat(CONTENT_LIMIT)).is_none());
        assert!(refusal(&"x".repeat(CONTENT_LIMIT + 1)).is_some());
    }
}

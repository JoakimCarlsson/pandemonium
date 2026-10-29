//! What the reader adds to the prompt from the browser pane besides picked elements: screenshots of the page and its console, as the helper reports them.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use pm_browser::{Capture as Reported, CaptureKind};
use pm_components::captures::{Capture, Kind, Shot};
use pm_components::state::PROMPT;

use super::App;

/// The folder under the data directory that pasted pictures, and the screenshots of pages, are saved in.
const PASTED: &str = "pasted";

impl App {
    /// Opens the menu of what can be added to the prompt from the page just under the pointer.
    pub(super) fn open_browser_capture_menu(&mut self) {
        let Some(at) = self.browser_menu_place() else {
            return;
        };
        self.model
            .update(|screen| *screen = std::mem::take(screen).with_browser_capture_menu(at));
    }

    /// Takes in what the helper reported for the reader's additions to the prompt: each becomes a chip, with the pictures attached from the window's own folder, and the previews are started.
    pub(super) fn browser_captured(&mut self) {
        for report in self.browser_reports() {
            let capture = capture_of(&report);
            let paths: Vec<String> = capture
                .as_ref()
                .map(|capture| capture.files().cloned().collect())
                .unwrap_or_default();
            let held = capture.is_some();
            self.model.update(|screen| {
                *screen = std::mem::take(screen).browser_captured(capture.clone());
            });
            if !paths.is_empty() {
                self.work.preview(paths);
            }
            if held {
                self.focus.focus(PROMPT);
            }
        }
    }
}

/// What the window holds of a report: the chip with its pictures moved out of the helper's folder, a failure when nothing could be taken, and nothing when the reader chose no area.
fn capture_of(report: &Reported) -> Option<Capture> {
    let kind = match report.kind {
        CaptureKind::Screenshot => Kind::Screenshot,
        CaptureKind::FullPage => Kind::FullPage,
        CaptureKind::Area => Kind::Area,
        CaptureKind::Console => Kind::Console,
    };
    let mut capture = Capture {
        kind,
        shots: Vec::new(),
        lines: report.lines.clone(),
        url: report.url.clone(),
        title: report.title.clone(),
        note: report.note.clone(),
    };
    for picture in &report.pictures {
        if let Some(path) = keep_picture(&picture.path) {
            capture.shots.push(Shot {
                label: picture.label.clone(),
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    let chose_nothing = kind == Kind::Area && report.pictures.is_empty() && report.error.is_empty();
    if chose_nothing {
        return None;
    }
    let nothing_taken = capture.shots.is_empty() && kind != Kind::Console;
    if !report.error.is_empty() || nothing_taken {
        capture.kind = Kind::Failed;
        capture.note = if report.error.is_empty() {
            "The picture could not be saved.".to_owned()
        } else {
            report.error.clone()
        };
    }
    Some(capture)
}

/// Moves the picture the helper saved at `source` into the pasted folder of the data directory, so that it outlives the helper's folder, and returns where it went.
fn keep_picture(source: &str) -> Option<PathBuf> {
    let source = PathBuf::from(source);
    let extension = source
        .extension()?
        .to_str()
        .filter(|extension| matches!(*extension, "png" | "jpg"))?
        .to_owned();
    let folder = pm_store::data_directory()?.join(PASTED);
    std::fs::create_dir_all(&folder).ok()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let target = folder.join(format!("page-{stamp}.{extension}"));
    if std::fs::rename(&source, &target).is_err() {
        std::fs::copy(&source, &target).ok()?;
        let _ = std::fs::remove_file(&source);
    }
    Some(target)
}

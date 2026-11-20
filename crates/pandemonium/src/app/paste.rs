//! Pasting files and pictures from the clipboard into the prompt as attachments.

use pm_components::areas::ComposerEvent;
use pm_components::target::Target;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::App;

/// The folder under the data directory that pasted pictures are saved in.
const PASTED: &str = "pasted";

impl App {
    /// Attaches the files or the picture on the clipboard to the prompt, and
    /// reports whether there were any; text is left for the caller to paste.
    pub(super) fn paste_attachments(&mut self) -> bool {
        let Some(clipboard) = self.clipboard() else {
            return false;
        };
        let files = Some(clipboard.files()).filter(|files| !files.is_empty());
        let paths: Vec<String> = match files {
            Some(files) => files
                .iter()
                .map(|file| file.to_string_lossy().into_owned())
                .collect(),
            None => match clipboard.image().and_then(save_picture) {
                Some(path) => vec![path.to_string_lossy().into_owned()],
                None => return false,
            },
        };
        self.attach_paths(paths);
        true
    }

    /// Attaches the files at `paths` to the prompt, starts their previews and focuses the prompt.
    pub(super) fn attach_paths(&mut self, paths: Vec<String>) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_attachments(&paths))
        });
        self.work.preview(paths);
        self.focus.focus(Target::Composer(ComposerEvent::Prompt));
    }
}

/// Saves `picture` as a PNG in the pasted folder of the data directory and returns where.
fn save_picture(picture: rustle::ClipboardImage) -> Option<PathBuf> {
    let pixels = image::RgbaImage::from_raw(picture.width, picture.height, picture.rgba)?;
    let folder = pm_store::data_directory()?.join(PASTED);
    std::fs::create_dir_all(&folder).ok()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    let path = folder.join(format!("pasted-{stamp}.png"));
    pixels.save(&path).ok()?;
    Some(path)
}

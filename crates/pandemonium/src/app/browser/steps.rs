//! The steps a browser recording kept, made ready for the panel that shows them: their screenshots read from the recording's folder and shrunk for the panel and its list.

use pm_browser::RecordedStep;
use pm_components::browser_steps::AgentStep;
use rustle::ImageData;

/// The longest side of the screenshot the panel shows, in pixels: twice its size in logical pixels, for sharp pictures on dense screens.
const PICTURE_SIDE: u32 = 640;

/// The longest side of a thumbnail of the list, in pixels.
const THUMBNAIL_SIDE: u32 = 160;

/// The step the helper reported, with its screenshot read from its file; the file stays, as part of the recording.
pub(super) fn agent_step(step: &RecordedStep) -> AgentStep {
    let decoded = (!step.picture.is_empty())
        .then(|| std::fs::read(&step.picture).ok())
        .flatten()
        .and_then(|bytes| image::load_from_memory(&bytes).ok());
    AgentStep {
        index: step.index,
        tool: step.tool.clone(),
        arguments: step.arguments.clone(),
        ok: step.ok,
        text: step.text.clone(),
        url: step.url.clone(),
        session: step.session.clone(),
        session_name: step.session_name.clone(),
        thumbnail: decoded
            .as_ref()
            .and_then(|picture| shrunk(picture, THUMBNAIL_SIDE)),
        picture: decoded
            .as_ref()
            .and_then(|picture| shrunk(picture, PICTURE_SIDE)),
    }
}

/// `picture` shrunk so its longest side is at most `side` pixels, as the panel draws it; nothing when it has no pixels.
fn shrunk(picture: &image::DynamicImage, side: u32) -> Option<ImageData> {
    let fitted = if picture.width().max(picture.height()) > side {
        picture.thumbnail(side, side)
    } else {
        picture.clone()
    };
    let rgba = fitted.to_rgba8();
    ImageData::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
}

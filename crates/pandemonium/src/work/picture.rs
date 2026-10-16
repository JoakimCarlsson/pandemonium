//! Pictures of open files turned into pixels away from the window: picture files decoded from their bytes and vector pictures drawn from their text.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use pm_components::preview::VECTOR_DENSITY;
use pm_renderer::Picture;

/// The longest side a picture is drawn or kept at, in pixels, so its texture fits any GPU.
const LONGEST_SIDE: u32 = 4096;

/// The shortest a vector picture's longest side is drawn at, in pixels, so a small icon is shown large enough to see.
const SHORTEST_VECTOR_SIDE: f32 = 512.0;

/// A key naming pixels made from `source` for the file at `path`, so the same pixels are uploaded once.
fn key_of(path: &str, source: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (path, source).hash(&mut hasher);
    hasher.finish()
}

/// The picture `bytes` of the file at `path` hold, decoded and shrunk to fit the longest side kept, or why it could not be read.
pub(crate) fn decode(path: &str, bytes: &[u8], truncated: bool) -> Result<Picture, String> {
    if truncated {
        return Err("This picture is too large to show.".to_owned());
    }
    let decoded = image::load_from_memory(bytes)
        .map_err(|error| format!("This picture could not be read: {error}"))?;
    let decoded = if decoded.width().max(decoded.height()) > LONGEST_SIDE {
        decoded.thumbnail(LONGEST_SIDE, LONGEST_SIDE)
    } else {
        decoded
    };
    let rgba = decoded.to_rgba8();
    Ok(Picture {
        key: key_of(path, bytes),
        width: rgba.width(),
        height: rgba.height(),
        pixels: Arc::new(rgba.into_raw()),
    })
}

/// The vector picture `text` of the file at `path` holds, drawn at the density of a drawn vector picture, or why it could not be drawn.
pub(crate) fn draw(path: &str, text: &str) -> Result<Picture, String> {
    let tree = resvg::usvg::Tree::from_str(text, &resvg::usvg::Options::default())
        .map_err(|error| format!("This picture could not be drawn: {error}"))?;
    let size = tree.size();
    let longest = size.width().max(size.height()).max(1.0);
    let side = (longest * VECTOR_DENSITY).clamp(SHORTEST_VECTOR_SIDE, LONGEST_SIDE as f32);
    let scale = side / longest;
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut canvas = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "This picture has no size to draw.".to_owned())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut canvas.as_mut(),
    );
    let pixels = canvas
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Ok(Picture {
        key: key_of(path, text.as_bytes()),
        width,
        height,
        pixels: Arc::new(pixels),
    })
}

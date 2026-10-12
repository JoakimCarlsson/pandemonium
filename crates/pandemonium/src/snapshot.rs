//! Drawing one frame to a PNG without opening a window.

use std::path::Path;

use pm_components::ShowcaseState;
use pm_input::{FocusId, PointerState};
use pm_renderer::{DrawList, Renderer};
use pm_text::Shaper;
use pm_ui::Input;

use crate::scene;

/// Logical width of the snapshot.
const WIDTH: u32 = 1280;

/// Logical height of the snapshot.
const HEIGHT: u32 = 800;

/// Draws the screen as it looks `elapsed_ms` into the animations, with the
/// pointer at `pointer` and focus on `focused` if given, and writes it to
/// `path` as a PNG.
pub fn write(
    path: &Path,
    elapsed_ms: f32,
    pointer: Option<(f32, f32)>,
    focused: Option<FocusId>,
) -> Result<(), String> {
    let mut renderer = Renderer::headless(WIDTH, HEIGHT, 1.0).map_err(|error| error.to_string())?;
    let mut shaper = Shaper::new();
    let mut list = DrawList::new();
    let input = Input {
        pointer: PointerState {
            position: pointer,
            ..PointerState::default()
        },
        focused,
        elapsed_ms,
    };
    scene::paint(
        WIDTH as f32,
        HEIGHT as f32,
        &ShowcaseState::default(),
        &input,
        &mut shaper,
        &mut list,
    );
    let pixels = renderer
        .snapshot(&list, &mut shaper, scene::GROUND)
        .ok_or("the renderer could not read its frame back")?;
    image::save_buffer(path, &pixels, WIDTH, HEIGHT, image::ColorType::Rgba8)
        .map_err(|error| error.to_string())
}

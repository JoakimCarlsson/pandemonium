//! Drawing a window's content once, offscreen, into a PNG file, through
//! rustle's [`preview_pixels`](rustle::preview_pixels): at a size and a scale,
//! at a fixed time on the frame's clock, with the pointer resting at a point
//! (so hover styles and tooltips show), a control holding focus (so its ring
//! and caret show), in the light or dark theme of a family. Edges are rounded
//! to whole logical pixels.

use pm_components::target::Target;
use std::time::Duration;

use pm_components::focus::{self, FocusKeys};
use pm_components::theme::{self, Appearance, ThemeFamily, Tokens};
use rustle::{Offset, PixelRounding, PreviewOptions};

/// How to draw one snapshot.
#[derive(Clone, Debug)]
pub(crate) struct Shot {
    /// Width in logical pixels.
    pub(crate) width: f32,
    /// Height in logical pixels.
    pub(crate) height: f32,
    /// Physical pixels per logical pixel.
    pub(crate) scale: f32,
    /// The time on the frame's clock, in milliseconds since the app started.
    pub(crate) clock_ms: f32,
    /// Where the pointer rests, in logical pixels, if over the window.
    pub(crate) pointer: Option<(f32, f32)>,
    /// The control holding keyboard focus.
    pub(crate) focused: Option<Target>,
    /// Whether what is not drawn stays see-through, as around a floating window's frame.
    pub(crate) transparent: bool,
    /// What an input method is composing in what holds the keyboard.
    pub(crate) preedit: Option<String>,
    /// The theme family.
    pub(crate) family: ThemeFamily,
    /// Light or dark.
    pub(crate) appearance: Appearance,
}

impl Shot {
    /// A snapshot `width` by `height` logical pixels at scale one, 700 ms in,
    /// with no pointer and no focus, in the dark theme of the default family.
    pub(crate) fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            scale: 1.0,
            clock_ms: 700.0,
            pointer: None,
            focused: None,
            transparent: false,
            preedit: None,
            family: theme::default_family(),
            appearance: Appearance::Dark,
        }
    }
}

/// Draws `content` as `shot` says into a PNG file at `path`, inside the
/// theme of `shot` and a root that shows tooltips at once, with the
/// app's focus on the shot's control and rustle's focus given to it.
///
/// # Errors
///
/// Returns an error when no GPU adapter can be opened, the picture cannot be
/// read back or encoded, or the file cannot be written.
pub(crate) fn write_png(
    path: &std::path::Path,
    shot: &Shot,
    content: impl Fn() + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let (width, height, rgba) = pixels(shot, content)?;
    let image = rustle::ImageData::from_rgba(width, height, rgba)
        .ok_or("the picture's size does not match its pixels")?;
    let png = image.encode_png()?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, png)?;
    Ok(())
}

/// Draws `content` as [`write_png`] does for `shot` and gives back the picture's width and
/// height in physical pixels and its RGBA bytes.
///
/// # Errors
///
/// Returns an error when no GPU adapter can be opened or the picture cannot be read back.
pub(crate) fn pixels(
    shot: &Shot,
    content: impl Fn() + 'static,
) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
    rustle_wgpu::set_legible_text(false);
    let tokens = Tokens::new(&shot.family, shot.appearance);
    let focused = shot.focused;
    let root = move || {
        focus::provide(FocusKeys::resting(focused), || {
            theme::provide(tokens.clone(), || {
                pm_components::tooltip::provide_tooltip_delay(0, || {
                    rustle::stack_with(rustle::Modifier::new().fill_max_size(), &content)
                })
            })
        })
    };
    let options = PreviewOptions {
        size: (shot.width, shot.height),
        scale: shot.scale,
        dark: shot.appearance == Appearance::Dark,
        clock: Duration::from_secs_f32(shot.clock_ms.max(0.0) / 1000.0),
        pointer: shot.pointer.map(|(x, y)| Offset::new(x, y)),
        transparent: shot.transparent,
        rounding: PixelRounding::Logical,
        focused_tag: shot.focused.map(Target::tag),
    };
    Ok(match &shot.preedit {
        Some(text) => rustle::preview_pixels_composing(
            root,
            options,
            &rustle::Preedit {
                text: text.clone(),
                cursor: Some((text.len(), text.len())),
            },
        )?,
        None => rustle::preview_pixels(root, options)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustle::{Color, Modifier, remember_mutable_interaction_source};

    /// A box 40 logical pixels square that fills itself red while the pointer is over it.
    fn hover_box() {
        let over = remember_mutable_interaction_source();
        let hovered = over.collect_is_hovered_as_state();
        rustle::spacer(
            Modifier::new()
                .width(40.0)
                .height(40.0)
                .hoverable(&over)
                .draw_behind(move |canvas| {
                    if hovered.get() {
                        canvas.rect(
                            0.0,
                            0.0,
                            canvas.width(),
                            canvas.height(),
                            Color::rgb(255, 0, 0),
                        );
                    }
                }),
        );
    }

    /// A shot with the pointer resting over a hoverable area draws its hover look, and one
    /// without a pointer does not.
    #[test]
    fn the_pointer_shows_hover() {
        let mut shot = Shot::new(80.0, 80.0);
        shot.pointer = Some((20.0, 20.0));
        let over = pixels(&shot, hover_box).expect("drawn");
        shot.pointer = None;
        let bare = pixels(&shot, hover_box).expect("drawn");
        assert_ne!(bare.2, over.2);
        let at = ((20 * over.0 + 20) * 4) as usize;
        assert_eq!(&over.2[at..at + 3], &[255, 0, 0]);
    }
}

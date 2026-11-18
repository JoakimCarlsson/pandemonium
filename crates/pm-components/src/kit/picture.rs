//! Pictures: decoded RGBA pixels drawn into a box, cropped to fill it.
//!
//! A [`Picture`] is named by a key; two pictures with one key are the same
//! pixels, so their texture is made once. A live surface, such as a page of
//! the embedded browser, draws a new frame under a [`Picture::live_key`] of
//! its surface and a growing revision; every revision of one surface is
//! drawn into the one texture rustle keeps for the surface's [`ImageKey`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use rustle::{ContentScale, ImageData, ImageKey, Modifier};

/// How many still pictures are kept ready to draw before the oldest are let go.
const KEPT: usize = 256;

/// The pixels of a picture, decoded and ready to be drawn.
#[derive(Clone, Debug)]
pub struct Picture {
    /// What names the picture; two pictures with one key are the same pixels.
    pub key: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Tightly packed sRGB-encoded RGBA bytes, row by row from the top.
    pub pixels: Arc<Vec<u8>>,
}

impl Picture {
    /// The bit of a key that marks a picture as one frame of a live surface.
    pub const LIVE: u64 = 1 << 63;

    /// How many low bits of a live key count the frames of its surface.
    const REVISION_BITS: u32 = 40;

    /// The key of frame `revision` of live surface `surface`, whose pixels replace the surface's last frame instead of being kept beside it.
    pub fn live_key(surface: u32, revision: u64) -> u64 {
        Self::LIVE
            | (u64::from(surface) << Self::REVISION_BITS)
            | (revision & ((1 << Self::REVISION_BITS) - 1))
    }

    /// The live surface a key belongs to, or none when the key names an ordinary picture.
    pub fn live_surface(key: u64) -> Option<u32> {
        (key & Self::LIVE != 0).then_some(((key & !Self::LIVE) >> Self::REVISION_BITS) as u32)
    }

    /// The rustle picture for these pixels: the same one for the same key, so
    /// its texture is kept, and for a live surface a new generation of the
    /// surface's one texture.
    pub fn image(&self) -> Option<ImageData> {
        CACHE.with(|cache| cache.borrow_mut().image(self))
    }
}

/// Pictures made ready to draw, by key, and the texture key of each live surface.
#[derive(Default)]
struct Cache {
    /// Still pictures by key, with when each was last used.
    still: HashMap<u64, (ImageData, u64)>,
    /// The texture key of each live surface, and its latest frame by key.
    live: HashMap<u32, (ImageKey, u64, ImageData)>,
    /// How many pictures were asked for, which orders their use.
    uses: u64,
}

impl Cache {
    /// The rustle picture for `picture`.
    fn image(&mut self, picture: &Picture) -> Option<ImageData> {
        self.uses += 1;
        if let Some(surface) = Picture::live_surface(picture.key) {
            if let Some((_, key, image)) = self.live.get(&surface)
                && *key == picture.key
            {
                return Some(image.clone());
            }
            let texture = self
                .live
                .get(&surface)
                .map_or_else(ImageKey::new, |(texture, ..)| *texture);
            let image = ImageData::keyed(
                texture,
                picture.width,
                picture.height,
                picture.pixels.as_slice(),
            )?;
            self.live
                .insert(surface, (texture, picture.key, image.clone()));
            return Some(image);
        }
        let uses = self.uses;
        if let Some((image, used)) = self.still.get_mut(&picture.key) {
            *used = uses;
            return Some(image.clone());
        }
        let image = ImageData::from_rgba(picture.width, picture.height, picture.pixels.to_vec())?;
        if self.still.len() >= KEPT
            && let Some(oldest) = self
                .still
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| *key)
        {
            self.still.remove(&oldest);
        }
        self.still.insert(picture.key, (image.clone(), uses));
        Some(image)
    }
}

thread_local! {
    /// The pictures made ready to draw on this thread.
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// Emits `picture` filling the box `modifier` sizes, cropped to fill it: a box with `.picture(picture)`.
#[track_caller]
pub fn picture(modifier: Modifier, picture: &Picture) {
    match picture.image() {
        Some(image) => rustle::image(modifier, &image, ContentScale::Crop),
        None => rustle::spacer(modifier),
    }
}

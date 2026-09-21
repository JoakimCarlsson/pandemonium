//! The shared glyph atlas: one grayscale texture every run draws from.

use std::collections::HashMap;

use cosmic_text::{CacheKey, SwashCache, SwashContent};

use crate::text::TextSystem;

/// Side length of the atlas texture in texels.
const ATLAS_SIZE: u32 = 2048;

/// Where one rasterized glyph landed in the atlas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphSlot {
    /// Column of the glyph's left edge in the atlas.
    pub x: u32,
    /// Row of the glyph's top edge in the atlas.
    pub y: u32,
    /// Width of the rasterized bitmap.
    pub width: u32,
    /// Height of the rasterized bitmap.
    pub height: u32,
    /// Bitmap left edge relative to the glyph's pen position.
    pub left: i32,
    /// Bitmap top edge relative to the glyph's baseline.
    pub top: i32,
}

/// Rasterizes glyphs on demand and packs them into one texture.
///
/// Space is handed out in shelves: a row grows to the tallest glyph in it, and
/// a glyph that does not fit starts the next row. Nothing is ever evicted; when
/// the texture is full, later glyphs are dropped rather than drawn wrongly.
pub(crate) struct GlyphAtlas {
    /// The rasterizer cache shared by every glyph.
    swash: SwashCache,
    /// The texture glyph bitmaps are written into.
    texture: wgpu::Texture,
    /// A view of the whole texture, for the glyph pipeline's bind group.
    view: wgpu::TextureView,
    /// Row the current shelf starts at.
    shelf_y: u32,
    /// Height of the current shelf.
    shelf_height: u32,
    /// Column the next glyph in this shelf is written at.
    next_x: u32,
    /// Slots already packed, and the glyphs found to have no bitmap at all.
    slots: HashMap<CacheKey, Option<GlyphSlot>>,
}

impl GlyphAtlas {
    /// Allocates the atlas texture on `device`.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            swash: SwashCache::new(),
            texture,
            view,
            shelf_y: 0,
            shelf_height: 0,
            next_x: 0,
            slots: HashMap::new(),
        }
    }

    /// The texture view the glyph pipeline samples.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Side length of the atlas in texels.
    pub(crate) fn size(&self) -> f32 {
        ATLAS_SIZE as f32
    }

    /// Returns the slot for `key`, rasterizing and packing it on first sight.
    pub(crate) fn slot(
        &mut self,
        text: &mut TextSystem,
        queue: &wgpu::Queue,
        key: CacheKey,
    ) -> Option<GlyphSlot> {
        if let Some(slot) = self.slots.get(&key) {
            return *slot;
        }

        let slot = self.rasterize(text, queue, key);
        self.slots.insert(key, slot);
        slot
    }

    /// Rasterizes one glyph and uploads its coverage bitmap.
    fn rasterize(
        &mut self,
        text: &mut TextSystem,
        queue: &wgpu::Queue,
        key: CacheKey,
    ) -> Option<GlyphSlot> {
        let image = self.swash.get_image_uncached(text.fonts_mut(), key)?;
        let width = image.placement.width;
        let height = image.placement.height;
        if width == 0 || height == 0 {
            return None;
        }

        let coverage = match image.content {
            SwashContent::Mask => image.data,
            SwashContent::SubpixelMask | SwashContent::Color => image
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|texel| texel[3])
                .collect(),
        };
        if coverage.len() < (width * height) as usize {
            return None;
        }

        let (x, y) = self.allocate(width, height)?;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Some(GlyphSlot {
            x,
            y,
            width,
            height,
            left: image.placement.left,
            top: image.placement.top,
        })
    }

    /// Reserves a `width` by `height` region, opening a new shelf when needed.
    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width > ATLAS_SIZE {
            return None;
        }

        if self.next_x + width > ATLAS_SIZE {
            self.shelf_y += self.shelf_height;
            self.shelf_height = 0;
            self.next_x = 0;
        }
        if self.shelf_y + height.max(self.shelf_height) > ATLAS_SIZE {
            return None;
        }

        let position = (self.next_x, self.shelf_y);
        self.next_x += width;
        self.shelf_height = self.shelf_height.max(height);
        Some(position)
    }
}

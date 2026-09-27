//! The shared coverage atlas: one grayscale texture every run draws from.
//!
//! Glyphs and icons live in it side by side, because they are drawn the same
//! way: a bitmap of coverage, tinted by the instance that samples it.

use std::collections::HashMap;

use cosmic_text::{CacheKey, SwashCache, SwashContent};

use crate::svg::{self, Svg};
use crate::text::TextSystem;

/// Side length the atlas texture starts at, in texels.
const INITIAL_SIDE: u32 = 2048;

/// Side length the atlas grows to at most, in texels; past it, a full atlas
/// is emptied instead.
const LARGEST_SIDE: u32 = 4096;

/// Why a bitmap could not be packed this time round.
///
/// Unlike artwork with no bitmap at all, this is not remembered: the atlas
/// makes room and the bitmap is packed the next time it is asked for.
#[derive(Debug)]
struct AtlasFull;

/// What a packed bitmap was rasterized from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SlotKey {
    /// One glyph at one size, as the shaper identified it.
    Glyph(CacheKey),
    /// One icon at one square size in texels.
    Icon(&'static str, u32),
}

/// Where one rasterized bitmap landed in the atlas.
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
/// a glyph that does not fit starts the next row. Nothing is evicted one at a
/// time. When a frame asks for more than the texture holds, the atlas says
/// so, and [`GlyphAtlas::make_room`] starts it over empty, larger if it can
/// still grow, so the frame can be built again with every glyph it needs
/// packed afresh.
pub(crate) struct GlyphAtlas {
    /// The rasterizer cache shared by every glyph.
    swash: SwashCache,
    /// The layout the atlas's bind group is made to.
    layout: wgpu::BindGroupLayout,
    /// How the glyph pipeline samples the texture.
    sampler: wgpu::Sampler,
    /// The texture glyph bitmaps are written into.
    texture: wgpu::Texture,
    /// The texture and its sampler, as the glyph pipeline's second group.
    group: wgpu::BindGroup,
    /// Side length of the texture in texels.
    side: u32,
    /// Row the current shelf starts at.
    shelf_y: u32,
    /// Height of the current shelf.
    shelf_height: u32,
    /// Column the next glyph in this shelf is written at.
    next_x: u32,
    /// Whether a bitmap has been turned away since the atlas last made room.
    overflowed: bool,
    /// Slots already packed, and the artwork found to have no bitmap at all.
    slots: HashMap<SlotKey, Option<GlyphSlot>>,
}

impl GlyphAtlas {
    /// Allocates the atlas texture on `device`.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glyph atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph atlas"),
            ..Default::default()
        });
        let side = INITIAL_SIDE.min(device.limits().max_texture_dimension_2d);
        let (texture, group) = Self::allocate_texture(device, &layout, &sampler, side);

        Self {
            swash: SwashCache::new(),
            layout,
            sampler,
            texture,
            group,
            side,
            shelf_y: 0,
            shelf_height: 0,
            next_x: 0,
            overflowed: false,
            slots: HashMap::new(),
        }
    }

    /// Creates a texture `side` texels square and the bind group sampling it.
    fn allocate_texture(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        side: u32,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
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
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph atlas"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        (texture, group)
    }

    /// The layout the glyph pipeline is built to.
    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The texture and its sampler, as the glyph pipeline binds them.
    pub(crate) fn group(&self) -> &wgpu::BindGroup {
        &self.group
    }

    /// Side length of the atlas in texels.
    pub(crate) fn size(&self) -> f32 {
        self.side as f32
    }

    /// Whether a bitmap has been turned away since the atlas last made room,
    /// so what was built from it is missing glyphs.
    pub(crate) fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Empties the atlas so every bitmap is packed again when next asked for,
    /// into a texture twice the size while it is still allowed to grow.
    ///
    /// The slots handed out before this are no longer valid, so whatever was
    /// built from them has to be built again.
    pub(crate) fn make_room(&mut self, device: &wgpu::Device) {
        let largest = LARGEST_SIDE.min(device.limits().max_texture_dimension_2d);
        if self.side < largest {
            self.side = (self.side * 2).min(largest);
            (self.texture, self.group) =
                Self::allocate_texture(device, &self.layout, &self.sampler, self.side);
        }
        self.shelf_y = 0;
        self.shelf_height = 0;
        self.next_x = 0;
        self.overflowed = false;
        self.slots.clear();
    }

    /// Returns the slot for `key`, rasterizing and packing it on first sight.
    pub(crate) fn slot(
        &mut self,
        text: &mut TextSystem,
        queue: &wgpu::Queue,
        key: CacheKey,
    ) -> Option<GlyphSlot> {
        if let Some(slot) = self.slots.get(&SlotKey::Glyph(key)) {
            return *slot;
        }

        self.remember(SlotKey::Glyph(key), |atlas| {
            atlas.rasterize(text, queue, key)
        })
    }

    /// Returns the slot for `svg` at `size` texels, rasterizing it once.
    pub(crate) fn icon_slot(
        &mut self,
        queue: &wgpu::Queue,
        svg: Svg,
        size: u32,
    ) -> Option<GlyphSlot> {
        let key = SlotKey::Icon(svg.name, size);
        if let Some(slot) = self.slots.get(&key) {
            return *slot;
        }

        self.remember(key, |atlas| atlas.rasterize_icon(queue, svg, size))
    }

    /// Packs what `pack` rasterizes under `key`, remembering the answer
    /// unless the atlas was too full to take it.
    fn remember(
        &mut self,
        key: SlotKey,
        pack: impl FnOnce(&mut Self) -> Result<Option<GlyphSlot>, AtlasFull>,
    ) -> Option<GlyphSlot> {
        match pack(self) {
            Ok(slot) => {
                self.slots.insert(key, slot);
                slot
            }
            Err(AtlasFull) => {
                self.overflowed = true;
                None
            }
        }
    }

    /// Rasterizes one icon and uploads its coverage bitmap.
    fn rasterize_icon(
        &mut self,
        queue: &wgpu::Queue,
        svg: Svg,
        size: u32,
    ) -> Result<Option<GlyphSlot>, AtlasFull> {
        let Some(coverage) = svg::rasterize(svg, size) else {
            return Ok(None);
        };
        let Some((x, y)) = self.allocate(size, size)? else {
            return Ok(None);
        };
        self.upload(queue, &coverage, x, y, size, size);

        Ok(Some(GlyphSlot {
            x,
            y,
            width: size,
            height: size,
            left: 0,
            top: 0,
        }))
    }

    /// Rasterizes one glyph and uploads its coverage bitmap.
    fn rasterize(
        &mut self,
        text: &mut TextSystem,
        queue: &wgpu::Queue,
        key: CacheKey,
    ) -> Result<Option<GlyphSlot>, AtlasFull> {
        let Some(image) = self.swash.get_image_uncached(text.fonts_mut(), key) else {
            return Ok(None);
        };
        let width = image.placement.width;
        let height = image.placement.height;
        if width == 0 || height == 0 {
            return Ok(None);
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
            return Ok(None);
        }

        let Some((x, y)) = self.allocate(width, height)? else {
            return Ok(None);
        };
        self.upload(queue, &coverage, x, y, width, height);

        Ok(Some(GlyphSlot {
            x,
            y,
            width,
            height,
            left: image.placement.left,
            top: image.placement.top,
        }))
    }
    /// Writes one coverage bitmap into the region reserved for it.
    fn upload(
        &self,
        queue: &wgpu::Queue,
        coverage: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            coverage,
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
    }

    /// Reserves a `width` by `height` region, opening a new shelf when needed.
    ///
    /// A bitmap larger than the atlas could ever grow to has no region at all;
    /// one that only needs the atlas emptied or grown first is turned away as
    /// [`AtlasFull`].
    fn allocate(&mut self, width: u32, height: u32) -> Result<Option<(u32, u32)>, AtlasFull> {
        if width > LARGEST_SIDE || height > LARGEST_SIDE {
            return Ok(None);
        }
        if width > self.side {
            return Err(AtlasFull);
        }

        if self.next_x + width > self.side {
            self.shelf_y += self.shelf_height;
            self.shelf_height = 0;
            self.next_x = 0;
        }
        if self.shelf_y + height.max(self.shelf_height) > self.side {
            return Err(AtlasFull);
        }

        let position = (self.next_x, self.shelf_y);
        self.next_x += width;
        self.shelf_height = self.shelf_height.max(height);
        Ok(Some(position))
    }
}

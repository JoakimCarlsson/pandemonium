//! The glyph atlas: coverage bitmaps packed into one texture.

use std::collections::HashMap;

use pm_text::{GlyphKey, Shaper};

/// The side of the atlas texture in pixels.
const SIZE: u32 = 2048;

/// Where one glyph's bitmap sits in the atlas and how it hangs off the pen.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    /// Left, top, right and bottom in normalized texture coordinates.
    pub(crate) uv: [f32; 4],
    /// The bitmap's width in pixels.
    pub(crate) width: f32,
    /// The bitmap's height in pixels.
    pub(crate) height: f32,
    /// How far right of the pen the left edge is.
    pub(crate) left: f32,
    /// How far above the baseline the top edge is.
    pub(crate) top: f32,
}

/// A texture of glyph coverage, filled in rows as glyphs are first drawn.
pub(crate) struct Atlas {
    /// The coverage texture.
    texture: wgpu::Texture,
    /// The bind group exposing the texture and its sampler.
    group: wgpu::BindGroup,
    /// The layout of [`Atlas::group`].
    layout: wgpu::BindGroupLayout,
    /// What has been rasterized, by glyph; `None` for glyphs with no ink.
    entries: HashMap<GlyphKey, Option<Entry>>,
    /// The next free pixel along the current row.
    cursor_x: u32,
    /// The top of the current row.
    cursor_y: u32,
    /// The tallest glyph in the current row.
    row_height: u32,
}

impl Atlas {
    /// An empty atlas on `device`.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph atlas"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
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
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph atlas"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            texture,
            group,
            layout,
            entries: HashMap::new(),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    /// The layout of the bind group a pipeline samples the atlas through.
    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The bind group exposing the atlas to a shader.
    pub(crate) fn group(&self) -> &wgpu::BindGroup {
        &self.group
    }

    /// Where `key` is in the atlas, rasterizing it on first use; nothing for
    /// a glyph with no ink or when the atlas is full.
    pub(crate) fn entry(
        &mut self,
        queue: &wgpu::Queue,
        shaper: &mut Shaper,
        key: GlyphKey,
    ) -> Option<Entry> {
        if let Some(entry) = self.entries.get(&key) {
            return *entry;
        }
        let entry = self.insert(queue, shaper, key);
        self.entries.insert(key, entry);
        entry
    }

    /// Rasterizes `key` and writes it into the next free spot.
    fn insert(&mut self, queue: &wgpu::Queue, shaper: &mut Shaper, key: GlyphKey) -> Option<Entry> {
        let bitmap = shaper.rasterize(key)?;
        if self.cursor_x + bitmap.width + 1 > SIZE {
            self.cursor_x = 0;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }
        if self.cursor_y + bitmap.height + 1 > SIZE {
            return None;
        }
        let (x, y) = (self.cursor_x, self.cursor_y);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap.coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width),
                rows_per_image: Some(bitmap.height),
            },
            wgpu::Extent3d {
                width: bitmap.width,
                height: bitmap.height,
                depth_or_array_layers: 1,
            },
        );
        self.cursor_x += bitmap.width + 1;
        self.row_height = self.row_height.max(bitmap.height);
        let size = SIZE as f32;
        Some(Entry {
            uv: [
                x as f32 / size,
                y as f32 / size,
                (x + bitmap.width) as f32 / size,
                (y + bitmap.height) as f32 / size,
            ],
            width: bitmap.width as f32,
            height: bitmap.height as f32,
            left: bitmap.left as f32,
            top: bitmap.top as f32,
        })
    }
}

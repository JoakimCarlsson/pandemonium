//! The device, the surface and the one frame each redraw presents.
//!
//! [`Renderer`] is the only thing in the workspace that holds a wgpu device.
//! Callers hand it a [`DrawList`] in logical pixels; scaling to physical
//! pixels, rasterizing glyphs and submitting the pass happen in here.

use std::collections::HashMap;
use std::ops::Range;

use crate::atlas::GlyphAtlas;
use crate::draw::{DrawList, Layer};
use crate::geometry::{Rect, Size};
use crate::pipeline::{GlyphInstance, InstanceBuffer, QuadInstance, Viewport, build_pipeline};
use crate::text::TextSystem;

/// The colour the window is cleared to before anything is drawn.
const GROUND: wgpu::Color = wgpu::Color {
    r: 0.0044,
    g: 0.0039,
    b: 0.0051,
    a: 1.0,
};

/// Owns the GPU device, queue and swapchain surface backing one window.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    scale: f32,
    text: TextSystem,
    atlas: GlyphAtlas,
    viewport_buffer: wgpu::Buffer,
    viewport_group: wgpu::BindGroup,
    atlas_group: wgpu::BindGroup,
    quad_pipeline: wgpu::RenderPipeline,
    quad_instances: InstanceBuffer,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_instances: InstanceBuffer,
}

impl Renderer {
    /// Creates a renderer drawing into `target` at the given pixel size.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Self {
        pollster::block_on(Self::create(target, width, height, scale))
    }

    /// Acquires an adapter and device, then configures the surface and pipelines.
    async fn create(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .expect("surface creation failed");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("pandemonium"),
                ..Default::default()
            })
            .await
            .expect("device request failed");

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: capabilities.present_modes[0],
            alpha_mode: capabilities.alpha_modes[0],
            color_space: wgpu::SurfaceColorSpace::Srgb,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport"),
            size: size_of::<Viewport>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let viewport_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport"),
            layout: &viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let atlas = GlyphAtlas::new(&device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph atlas"),
            ..Default::default()
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let atlas_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph atlas"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let quad_pipeline = build_pipeline(
            &device,
            format,
            "quad",
            include_str!("shaders/quad.wgsl"),
            &[Some(&viewport_layout)],
            size_of::<QuadInstance>() as u64,
            &QuadInstance::ATTRIBUTES,
        );
        let glyph_pipeline = build_pipeline(
            &device,
            format,
            "glyph",
            include_str!("shaders/glyph.wgsl"),
            &[Some(&viewport_layout), Some(&atlas_layout)],
            size_of::<GlyphInstance>() as u64,
            &GlyphInstance::ATTRIBUTES,
        );

        Self {
            surface,
            device,
            queue,
            config,
            scale,
            text: TextSystem::new(),
            atlas,
            viewport_buffer,
            viewport_group,
            atlas_group,
            quad_pipeline,
            quad_instances: InstanceBuffer::new("quad instances"),
            glyph_pipeline,
            glyph_instances: InstanceBuffer::new("glyph instances"),
        }
    }

    /// The text system layout measures with and draw lists shape through.
    pub fn text(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    /// The surface size in logical pixels.
    pub fn size(&self) -> Size {
        Size::new(
            self.config.width as f32 / self.scale,
            self.config.height as f32 / self.scale,
        )
    }

    /// Reconfigures the surface for a new physical size and scale factor.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.scale = scale.max(f32::EPSILON);
        self.surface.configure(&self.device, &self.config);
    }

    /// Draws one frame of `list` and presents it.
    pub fn render(&mut self, list: &DrawList) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };

        self.queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::bytes_of(&Viewport {
                size: [self.config.width as f32, self.config.height as f32],
                padding: [0.0; 2],
            }),
        );

        let (quads, quad_layers) = sorted(self.build_quads(list));
        let mut glyphs = self.build_glyphs(list);
        glyphs.extend(self.build_icons(list));
        let (glyphs, glyph_layers) = sorted(glyphs);
        self.quad_instances
            .upload(&self.device, &self.queue, bytemuck::cast_slice(&quads));
        self.glyph_instances
            .upload(&self.device, &self.queue, bytemuck::cast_slice(&glyphs));

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GROUND),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.viewport_group, &[]);
            for layer in list.layers() {
                if let Some(buffer) = self.quad_instances.buffer()
                    && let Some(range) = quad_layers.get(&layer)
                {
                    pass.set_pipeline(&self.quad_pipeline);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, range.clone());
                }
                if let Some(buffer) = self.glyph_instances.buffer()
                    && let Some(range) = glyph_layers.get(&layer)
                {
                    pass.set_pipeline(&self.glyph_pipeline);
                    pass.set_bind_group(1, &self.atlas_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, range.clone());
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }

    /// Converts the list's quads to physical-pixel instances.
    fn build_quads(&self, list: &DrawList) -> Vec<(Layer, QuadInstance)> {
        list.quads()
            .iter()
            .map(|(quad, clip, layer)| {
                (
                    *layer,
                    QuadInstance {
                        origin: [
                            quad.bounds.left() * self.scale,
                            quad.bounds.top() * self.scale,
                        ],
                        size: [
                            quad.bounds.size.width * self.scale,
                            quad.bounds.size.height * self.scale,
                        ],
                        background: quad.background.to_array(),
                        border_color: quad.border_color.to_array(),
                        radii: quad.corner_radii.map(|radius| radius * self.scale),
                        border: [quad.border_width * self.scale, 0.0],
                        clip: self.clip(*clip),
                    },
                )
            })
            .collect()
    }

    /// Rasterizes the list's text and converts it to glyph instances.
    fn build_glyphs(&mut self, list: &DrawList) -> Vec<(Layer, GlyphInstance)> {
        let scale = self.scale;
        let atlas_size = self.atlas.size();
        let mut instances = Vec::new();

        for (text, clip, layer) in list.texts() {
            let clip = self.clip(*clip);
            let color = text.color.to_array();
            let offset = (
                (text.origin.x * scale).round(),
                ((text.origin.y + text.run.baseline) * scale).round(),
            );

            for glyph in &text.run.glyphs {
                let physical = glyph.physical(offset, scale);
                let Some(slot) = self
                    .atlas
                    .slot(&mut self.text, &self.queue, physical.cache_key)
                else {
                    continue;
                };

                instances.push((
                    *layer,
                    GlyphInstance {
                        origin: [
                            (physical.x + slot.left) as f32,
                            (physical.y - slot.top) as f32,
                        ],
                        size: [slot.width as f32, slot.height as f32],
                        uv_origin: [slot.x as f32 / atlas_size, slot.y as f32 / atlas_size],
                        uv_size: [
                            slot.width as f32 / atlas_size,
                            slot.height as f32 / atlas_size,
                        ],
                        color,
                        clip,
                    },
                ));
            }
        }

        instances
    }

    /// Rasterizes the list's icons and converts them to glyph instances.
    ///
    /// An icon is a glyph as far as the GPU is concerned: the same atlas, the
    /// same pipeline, the same tint. What differs is only where the coverage
    /// came from, which the atlas has already forgotten by this point.
    fn build_icons(&mut self, list: &DrawList) -> Vec<(Layer, GlyphInstance)> {
        let scale = self.scale;
        let atlas_size = self.atlas.size();
        let mut instances = Vec::new();

        for (icon, clip, layer) in list.icons() {
            let side = (icon.bounds.size.width.min(icon.bounds.size.height) * scale).round();
            let Some(slot) = self.atlas.icon_slot(&self.queue, icon.svg, side as u32) else {
                continue;
            };

            instances.push((
                *layer,
                GlyphInstance {
                    origin: [
                        (icon.bounds.left() * scale).round(),
                        (icon.bounds.top() * scale).round(),
                    ],
                    size: [slot.width as f32, slot.height as f32],
                    uv_origin: [slot.x as f32 / atlas_size, slot.y as f32 / atlas_size],
                    uv_size: [
                        slot.width as f32 / atlas_size,
                        slot.height as f32 / atlas_size,
                    ],
                    color: icon.color.to_array(),
                    clip: self.clip(*clip),
                },
            ));
        }

        instances
    }

    /// Converts a logical clip rectangle to the physical bounds shaders test.
    fn clip(&self, clip: Rect) -> [f32; 4] {
        [
            clip.left() * self.scale,
            clip.top() * self.scale,
            clip.right() * self.scale,
            clip.bottom() * self.scale,
        ]
    }
}

/// Orders `instances` by layer and says which range of them each layer is.
///
/// The instances of one layer end up next to each other, so a layer is one
/// draw call rather than one per primitive, and the layers are drawn lowest
/// first because that is what being over something means.
fn sorted<T>(instances: Vec<(Layer, T)>) -> (Vec<T>, HashMap<u32, Range<u32>>) {
    let mut instances = instances;
    instances.sort_by_key(|(layer, _)| *layer);

    let mut ranges: HashMap<u32, Range<u32>> = HashMap::new();
    for (index, (layer, _)) in instances.iter().enumerate() {
        let index = index as u32;
        ranges
            .entry(layer.0)
            .and_modify(|range| range.end = index + 1)
            .or_insert(index..index + 1);
    }

    (
        instances
            .into_iter()
            .map(|(_, instance)| instance)
            .collect(),
        ranges,
    )
}

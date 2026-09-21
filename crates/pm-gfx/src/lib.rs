//! GPU device, glyph atlas, text shaping and draw-list submission.
//!
//! Callers build a [`DrawList`] of quads and shaped text in logical pixels and
//! hand it to [`Renderer::render`]. Queues, encoders, pipelines and bind groups
//! stay behind this boundary.

mod atlas;
mod color;
mod draw;
mod geometry;
mod text;

pub use color::Rgba;
pub use draw::{DrawList, Quad, TextRun};
pub use geometry::{Point, Rect, Size};
pub use text::{FontStyle, ShapedRun, TextSystem};

use atlas::GlyphAtlas;

/// The colour the window is cleared to before anything is drawn.
const GROUND: wgpu::Color = wgpu::Color {
    r: 0.0044,
    g: 0.0039,
    b: 0.0051,
    a: 1.0,
};

/// The viewport size every shader turns pixels into clip space with.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Viewport {
    /// Surface size in physical pixels.
    size: [f32; 2],
    /// Padding to the 16-byte alignment a uniform block needs.
    padding: [f32; 2],
}

/// One rounded rectangle, in physical pixels, as the quad pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadInstance {
    /// Top-left corner.
    origin: [f32; 2],
    /// Extent from the origin.
    size: [f32; 2],
    /// Fill colour in linear light.
    background: [f32; 4],
    /// Border colour in linear light.
    border_color: [f32; 4],
    /// Corner radii, clockwise from the top-left corner.
    radii: [f32; 4],
    /// Border width, and padding to the next attribute.
    border: [f32; 2],
    /// Clip rectangle as left, top, right, bottom.
    clip: [f32; 4],
}

/// One glyph, in physical pixels, as the glyph pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphInstance {
    /// Top-left corner of the glyph's bitmap.
    origin: [f32; 2],
    /// Size of the glyph's bitmap.
    size: [f32; 2],
    /// Top-left corner in atlas texture coordinates.
    uv_origin: [f32; 2],
    /// Extent in atlas texture coordinates.
    uv_size: [f32; 2],
    /// Tint in linear light.
    color: [f32; 4],
    /// Clip rectangle as left, top, right, bottom.
    clip: [f32; 4],
}

/// A vertex buffer that grows to fit whatever the frame holds.
struct InstanceBuffer {
    /// Debug label carried onto every reallocation.
    label: &'static str,
    /// The buffer currently allocated, or none before the first upload.
    buffer: Option<wgpu::Buffer>,
    /// Size of that buffer in bytes.
    capacity: usize,
}

impl InstanceBuffer {
    /// Creates an empty buffer that will be labelled `label`.
    fn new(label: &'static str) -> Self {
        Self {
            label,
            buffer: None,
            capacity: 0,
        }
    }

    /// Uploads `data`, reallocating when it no longer fits.
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        if self.capacity < data.len() {
            self.capacity = data.len().next_power_of_two();
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: self.capacity as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let buffer = self.buffer.as_ref().expect("buffer allocated above");
        queue.write_buffer(buffer, 0, data);
    }

    /// The allocated buffer, once something has been uploaded.
    fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }
}

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
            .find(wgpu::TextureFormat::is_srgb)
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
            &wgpu::vertex_attr_array![
                0 => Float32x2,
                1 => Float32x2,
                2 => Float32x4,
                3 => Float32x4,
                4 => Float32x4,
                5 => Float32x2,
                6 => Float32x4,
            ],
        );
        let glyph_pipeline = build_pipeline(
            &device,
            format,
            "glyph",
            include_str!("shaders/glyph.wgsl"),
            &[Some(&viewport_layout), Some(&atlas_layout)],
            size_of::<GlyphInstance>() as u64,
            &wgpu::vertex_attr_array![
                0 => Float32x2,
                1 => Float32x2,
                2 => Float32x2,
                3 => Float32x2,
                4 => Float32x4,
                5 => Float32x4,
            ],
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

        let quads = self.build_quads(list);
        let glyphs = self.build_glyphs(list);
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
            if let Some(buffer) = self.quad_instances.buffer().filter(|_| !quads.is_empty()) {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..quads.len() as u32);
            }
            if let Some(buffer) = self.glyph_instances.buffer().filter(|_| !glyphs.is_empty()) {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(1, &self.atlas_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..glyphs.len() as u32);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }

    /// Converts the list's quads to physical-pixel instances.
    fn build_quads(&self, list: &DrawList) -> Vec<QuadInstance> {
        list.quads()
            .iter()
            .map(|(quad, clip)| QuadInstance {
                origin: [
                    quad.bounds.left() * self.scale,
                    quad.bounds.top() * self.scale,
                ],
                size: [
                    quad.bounds.size.width * self.scale,
                    quad.bounds.size.height * self.scale,
                ],
                background: quad.background.to_linear(),
                border_color: quad.border_color.to_linear(),
                radii: quad.corner_radii.map(|radius| radius * self.scale),
                border: [quad.border_width * self.scale, 0.0],
                clip: self.clip(*clip),
            })
            .collect()
    }

    /// Rasterizes the list's text and converts it to glyph instances.
    fn build_glyphs(&mut self, list: &DrawList) -> Vec<GlyphInstance> {
        let scale = self.scale;
        let atlas_size = self.atlas.size();
        let mut instances = Vec::new();

        for (text, clip) in list.texts() {
            let clip = self.clip(*clip);
            let color = text.color.to_linear();
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

                instances.push(GlyphInstance {
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
                });
            }
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

/// Builds one instanced pipeline from a shader that draws six vertices a quad.
fn build_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    label: &'static str,
    source: &str,
    bind_group_layouts: &[Option<&wgpu::BindGroupLayout>],
    instance_stride: u64,
    attributes: &[wgpu::VertexAttribute],
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts,
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: instance_stride,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes,
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

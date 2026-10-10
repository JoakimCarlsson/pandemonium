//! The device, the surface and the one frame each redraw presents.
//!
//! [`Renderer`] is the only thing in the workspace that holds a wgpu device.
//! Callers hand it a [`DrawList`] in logical pixels; scaling to physical
//! pixels, rasterizing glyphs and submitting the pass happen in here.

use std::ops::Range;

use crate::atlas::GlyphAtlas;
use crate::draw::{DrawList, Layer};
use crate::geometry::{Rect, Size};
use crate::pipeline::{
    GlyphInstance, ImageInstance, InstanceBuffer, QuadInstance, Viewport, build_pipeline,
};
use crate::text::TextSystem;
use crate::textures::Textures;

/// The colour the window is cleared to before anything is drawn.
const GROUND: wgpu::Color = wgpu::Color {
    r: 0.0044,
    g: 0.0039,
    b: 0.0051,
    a: 1.0,
};

/// Why a [`Renderer`] could not be built for a window.
#[derive(Debug)]
pub enum RendererError {
    /// The window offered no surface wgpu can present to.
    Surface(wgpu::CreateSurfaceError),
    /// No GPU adapter can drive the window's surface.
    Adapter(wgpu::RequestAdapterError),
    /// The adapter refused to open a device.
    Device(wgpu::RequestDeviceError),
}

impl std::fmt::Display for RendererError {
    /// Says what failed in words a reader without the source can act on.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(error) => {
                write!(f, "the window has no surface the GPU can draw to: {error}")
            }
            Self::Adapter(error) => write!(
                f,
                "no GPU adapter can drive this window; check that Vulkan, Metal or DirectX 12 drivers are installed: {error}"
            ),
            Self::Device(error) => write!(f, "the GPU refused to open a device: {error}"),
        }
    }
}

impl std::error::Error for RendererError {
    /// The wgpu error underneath.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::Adapter(error) => Some(error),
            Self::Device(error) => Some(error),
        }
    }
}

/// Owns the GPU device, queue and swapchain surface backing one window.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The physical size the swapchain was last configured to present.
    configured_size: (u32, u32),
    scale: f32,
    text: TextSystem,
    atlas: GlyphAtlas,
    viewport_buffer: wgpu::Buffer,
    viewport_group: wgpu::BindGroup,
    quad_pipeline: wgpu::RenderPipeline,
    quad_instances: InstanceBuffer,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_instances: InstanceBuffer,
    textures: Textures,
    image_pipeline: wgpu::RenderPipeline,
    image_instances: InstanceBuffer,
    quads: Batch<QuadInstance>,
    glyphs: Batch<GlyphInstance>,
    images: Batch<(ImageInstance, u64)>,
    image_upload: Vec<ImageInstance>,
}

/// One kind of instance for one frame, gathered in the order it was pushed
/// and then grouped by layer.
///
/// A batch lives on the renderer and is emptied rather than dropped, so a
/// frame reuses the allocations of the one before it.
struct Batch<T> {
    /// The instances as they were pushed, each with the layer it went into.
    pushed: Vec<(Layer, T)>,
    /// The instances grouped by layer, lowest first, in pushed order within
    /// a layer.
    grouped: Vec<T>,
    /// Which run of `grouped` each layer is, indexed by layer.
    layers: Vec<Range<u32>>,
}

impl<T: Copy> Batch<T> {
    /// An empty batch.
    fn new() -> Self {
        Self {
            pushed: Vec::new(),
            grouped: Vec::new(),
            layers: Vec::new(),
        }
    }

    /// Empties the batch for the next frame, keeping its allocations.
    fn clear(&mut self) {
        self.pushed.clear();
        self.grouped.clear();
        self.layers.clear();
    }

    /// Adds `instance` to `layer`.
    fn push(&mut self, layer: Layer, instance: T) {
        self.pushed.push((layer, instance));
    }

    /// Groups the pushed instances by layer, for a frame with `count` layers.
    ///
    /// The instances of one layer end up next to each other, so a layer is
    /// one draw call rather than one per primitive, and the order they were
    /// pushed in survives inside it, because that is what drawn over means.
    /// Layers are few and numbered from zero, so this counts rather than sorts.
    fn group(&mut self, count: u32) {
        self.layers.clear();
        self.layers.resize(count as usize, 0..0);
        for (layer, _) in &self.pushed {
            self.layers[layer.0 as usize].end += 1;
        }
        let mut start = 0;
        for range in &mut self.layers {
            let length = range.end;
            *range = start..start;
            start += length;
        }

        self.grouped.clear();
        let Some(&(_, filler)) = self.pushed.first() else {
            return;
        };
        self.grouped.resize(self.pushed.len(), filler);
        for (layer, instance) in &self.pushed {
            let range = &mut self.layers[layer.0 as usize];
            self.grouped[range.end as usize] = *instance;
            range.end += 1;
        }
    }

    /// The run of grouped instances in `layer`, when it has any.
    fn layer(&self, layer: u32) -> Option<Range<u32>> {
        self.layers
            .get(layer as usize)
            .filter(|range| !range.is_empty())
            .cloned()
    }
}

impl Renderer {
    /// Creates a renderer drawing into `target` at the given pixel size.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        pollster::block_on(Self::create(target, width, height, scale))
    }

    /// Acquires an adapter and device, then configures the surface and pipelines.
    async fn create(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .map_err(RendererError::Surface)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(RendererError::Adapter)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("pandemonium"),
                ..Default::default()
            })
            .await
            .map_err(RendererError::Device)?;

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
            present_mode: wgpu::PresentMode::AutoVsync,
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
            &[Some(&viewport_layout), Some(atlas.layout())],
            size_of::<GlyphInstance>() as u64,
            &GlyphInstance::ATTRIBUTES,
        );

        let textures = Textures::new(&device);
        let image_pipeline = build_pipeline(
            &device,
            format,
            "image",
            include_str!("shaders/image.wgsl"),
            &[Some(&viewport_layout), Some(textures.layout())],
            size_of::<ImageInstance>() as u64,
            &ImageInstance::ATTRIBUTES,
        );

        Ok(Self {
            surface,
            device,
            queue,
            configured_size: (config.width, config.height),
            config,
            scale,
            text: TextSystem::new(),
            atlas,
            viewport_buffer,
            viewport_group,
            quad_pipeline,
            quad_instances: InstanceBuffer::new("quad instances"),
            glyph_pipeline,
            glyph_instances: InstanceBuffer::new("glyph instances"),
            textures,
            image_pipeline,
            image_instances: InstanceBuffer::new("image instances"),
            quads: Batch::new(),
            glyphs: Batch::new(),
            images: Batch::new(),
            image_upload: Vec::new(),
        })
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

    /// Records the latest physical size and scale factor for the next frame.
    ///
    /// Resize events can arrive faster than frames are presented. Updating
    /// the logical size immediately keeps layout current while deferring
    /// swapchain recreation until a frame needs the final physical size.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.scale = scale.max(f32::EPSILON);
    }

    /// Recreates the swapchain at the latest requested physical size.
    fn configure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
        self.configured_size = (self.config.width, self.config.height);
    }

    /// Draws one frame of `list` and presents it, notifying the window through
    /// `before_present` immediately before submitting the frame to the display.
    ///
    /// A surface that timed out or is hidden skips the frame and is asked
    /// again on the next one; only a surface that is outdated or lost is
    /// configured again, along with a changed physical size.
    pub fn render(&mut self, list: &DrawList, before_present: impl FnOnce()) {
        self.text.end_frame();
        if self.configured_size != (self.config.width, self.config.height) {
            self.configure_surface();
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };

        self.queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::bytes_of(&Viewport {
                size: [self.config.width as f32, self.config.height as f32],
                padding: [0.0; 2],
            }),
        );

        let layers = list.layers().end() + 1;
        let mut quads = std::mem::replace(&mut self.quads, Batch::new());
        let mut glyphs = std::mem::replace(&mut self.glyphs, Batch::new());
        let mut images = std::mem::replace(&mut self.images, Batch::new());

        self.build_quads(list, &mut quads);
        self.build_coverage(list, &mut glyphs);
        if self.atlas.overflowed() {
            self.atlas.make_room(&self.device);
            self.build_coverage(list, &mut glyphs);
        }
        self.build_images(list, &mut images);
        quads.group(layers);
        glyphs.group(layers);
        images.group(layers);

        self.quad_instances
            .upload(&self.device, &self.queue, &quads.grouped);
        self.glyph_instances
            .upload(&self.device, &self.queue, &glyphs.grouped);
        self.image_upload.clear();
        self.image_upload
            .extend(images.grouped.iter().map(|(instance, _)| *instance));
        self.image_instances
            .upload(&self.device, &self.queue, &self.image_upload);

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
                if let Some(range) = quads.layer(layer) {
                    pass.set_pipeline(&self.quad_pipeline);
                    for (buffer, local, _) in self.quad_instances.slices(range) {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..6, local);
                    }
                }
                if let Some(range) = images.layer(layer) {
                    pass.set_pipeline(&self.image_pipeline);
                    for (buffer, local, base) in self.image_instances.slices(range) {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        for index in local {
                            let (_, picture) = images.grouped[base + index as usize];
                            let Some(group) = self.textures.group(picture) else {
                                continue;
                            };
                            pass.set_bind_group(1, group, &[]);
                            pass.draw(0..6, index..index + 1);
                        }
                    }
                }
                if let Some(range) = glyphs.layer(layer) {
                    pass.set_pipeline(&self.glyph_pipeline);
                    pass.set_bind_group(1, self.atlas.group(), &[]);
                    for (buffer, local, _) in self.glyph_instances.slices(range) {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..6, local);
                    }
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        before_present();
        self.queue.present(frame);

        self.quads = quads;
        self.glyphs = glyphs;
        self.images = images;
    }

    /// Converts the list's quads to physical-pixel instances.
    fn build_quads(&self, list: &DrawList, batch: &mut Batch<QuadInstance>) {
        batch.clear();
        for (quad, clip, layer) in list.quads() {
            batch.push(
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
            );
        }
    }

    /// Rasterizes the list's text and icons and converts them to glyph
    /// instances, starting the batch over.
    ///
    /// When the atlas fills part way through, the batch is missing glyphs;
    /// the caller makes room in the atlas and builds it again.
    fn build_coverage(&mut self, list: &DrawList, batch: &mut Batch<GlyphInstance>) {
        batch.clear();
        self.build_glyphs(list, batch);
        self.build_icons(list, batch);
    }

    /// Rasterizes the list's text and converts it to glyph instances.
    fn build_glyphs(&mut self, list: &DrawList, batch: &mut Batch<GlyphInstance>) {
        let scale = self.scale;
        let atlas_size = self.atlas.size();

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

                let origin = [
                    physical.x as f32 + slot.left as f32,
                    physical.y as f32 - slot.top as f32,
                ];
                if origin[0] >= clip[2]
                    || origin[1] >= clip[3]
                    || origin[0] + slot.width as f32 <= clip[0]
                    || origin[1] + slot.height as f32 <= clip[1]
                {
                    continue;
                }

                batch.push(
                    *layer,
                    GlyphInstance {
                        origin,
                        size: [slot.width as f32, slot.height as f32],
                        uv_origin: [slot.x as f32 / atlas_size, slot.y as f32 / atlas_size],
                        uv_size: [
                            slot.width as f32 / atlas_size,
                            slot.height as f32 / atlas_size,
                        ],
                        color,
                        clip,
                        rotation: [0.0; 4],
                    },
                );
            }
        }
    }

    /// Rasterizes the list's icons and converts them to glyph instances.
    ///
    /// An icon is a glyph as far as the GPU is concerned: the same atlas, the
    /// same pipeline, the same tint. What differs is only where the coverage
    /// came from, which the atlas has already forgotten by this point.
    fn build_icons(&mut self, list: &DrawList, batch: &mut Batch<GlyphInstance>) {
        let scale = self.scale;
        let atlas_size = self.atlas.size();

        for (icon, clip, layer) in list.icons() {
            let side = (icon.bounds.size.width.min(icon.bounds.size.height) * scale).round();
            let Some(slot) = self.atlas.icon_slot(&self.queue, icon.svg, side as u32) else {
                continue;
            };

            batch.push(
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
                    rotation: [icon.rotation, 0.0, 0.0, 0.0],
                },
            );
        }
    }

    /// Uploads the list's pictures and converts them to image instances, each
    /// with the identity of the texture it samples.
    fn build_images(&mut self, list: &DrawList, batch: &mut Batch<(ImageInstance, u64)>) {
        self.textures.keep(
            &self.device,
            &self.queue,
            list.images().iter().map(|(run, _, _)| &run.image),
        );

        batch.clear();
        for (run, clip, layer) in list.images() {
            batch.push(
                *layer,
                (
                    ImageInstance {
                        origin: [
                            run.bounds.left() * self.scale,
                            run.bounds.top() * self.scale,
                        ],
                        size: [
                            run.bounds.size.width * self.scale,
                            run.bounds.size.height * self.scale,
                        ],
                        clip: self.clip(*clip),
                    },
                    run.image.id(),
                ),
            );
        }
    }

    /// Converts a logical clip rectangle to the physical bounds shaders test.
    fn clip(&self, clip: Rect) -> [f32; 4] {
        [
            (clip.left() * self.scale).max(0.0),
            (clip.top() * self.scale).max(0.0),
            (clip.right() * self.scale).min(self.config.width as f32),
            (clip.bottom() * self.scale).min(self.config.height as f32),
        ]
    }
}

//! The device, the target and the one frame each redraw presents.

use std::collections::HashMap;
use std::ops::Range;

use pm_color::Srgba;
use pm_text::Shaper;

use crate::atlas::Atlas;
use crate::draw::{DrawList, Mask, Picture, Primitive};
use crate::geometry::Rect;
use crate::pipeline::{GlyphInstance, QuadInstance, Viewport, build_pipeline};

/// The format an offscreen target is drawn in and read back as.
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Why a [`Renderer`] could not be built.
#[derive(Debug)]
pub enum RendererError {
    /// The window offered no surface wgpu can present to.
    Surface(wgpu::CreateSurfaceError),
    /// No GPU adapter can drive the target.
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

/// What a frame is drawn into.
enum Target {
    /// A window's swapchain.
    Surface {
        /// The window's swapchain surface.
        surface: wgpu::Surface<'static>,
        /// How the surface is configured at the latest requested size.
        config: wgpu::SurfaceConfiguration,
        /// The physical size the swapchain was last configured to present.
        configured_size: (u32, u32),
    },
    /// A texture that is read back instead of presented.
    Offscreen {
        /// The texture frames are drawn into.
        texture: wgpu::Texture,
        /// Its physical size.
        size: (u32, u32),
    },
}

/// Owns the GPU device and queue and the target they draw into.
pub struct Renderer {
    /// The wgpu instance the device was found through, which surfaces of other windows are made with.
    instance: wgpu::Instance,
    /// The GPU adapter the device was opened on.
    adapter: wgpu::Adapter,
    /// The opened GPU device.
    device: wgpu::Device,
    /// The queue frames are submitted to.
    queue: wgpu::Queue,
    /// What frames are drawn into.
    target: Target,
    /// Physical pixels per logical pixel.
    scale: f32,
    /// How wide the band around the drawing is that is left for what spills out of it, such as the shadow of a window, in logical pixels.
    band: f32,
    /// The layout of the bind group exposing a viewport to the shaders.
    viewport_layout: wgpu::BindGroupLayout,
    /// The uniform holding each viewport of the frame and the bind group exposing it: the first masks nothing, and each one after it masks to one rounded rectangle of the frame.
    viewports: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    /// The pipeline that draws quads.
    quad_pipeline: wgpu::RenderPipeline,
    /// The pipeline that draws glyphs.
    glyph_pipeline: wgpu::RenderPipeline,
    /// The pipeline that draws pictures.
    image_pipeline: wgpu::RenderPipeline,
    /// The layout of the bind group a picture is sampled through.
    image_layout: wgpu::BindGroupLayout,
    /// The sampler pictures are drawn with.
    image_sampler: wgpu::Sampler,
    /// The bind group of every picture uploaded so far, by key.
    images: HashMap<u64, wgpu::BindGroup>,
    /// The texture and bind group each live surface draws from, and the key of the frame in them.
    live: HashMap<u32, Live>,
    /// The buffer holding the picture instances of the frame being drawn.
    image_instances: Option<wgpu::Buffer>,
    /// The coverage of every glyph drawn so far.
    atlas: Atlas,
    /// The buffer holding the quad instances of the frame being drawn.
    quad_instances: Option<wgpu::Buffer>,
    /// The buffer holding the glyph instances of the frame being drawn.
    glyph_instances: Option<wgpu::Buffer>,
}

/// What a live surface keeps on the GPU between frames.
struct Live {
    /// The texture its frames are written into.
    texture: wgpu::Texture,
    /// The size of the texture, in pixels.
    size: (u32, u32),
    /// What the pipeline draws it with.
    group: wgpu::BindGroup,
    /// The key of the frame the texture holds.
    key: u64,
}

/// Which pipeline a run of instances is drawn with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// Rounded rectangles.
    Quad,
    /// Glyphs.
    Glyph,
    /// The picture with this key.
    Image(u64),
}

/// Writes `bytes` into `slot`, growing the buffer when it is too small.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<wgpu::Buffer>,
    label: &'static str,
    bytes: &[u8],
) {
    if bytes.is_empty() {
        return;
    }
    if slot
        .as_ref()
        .is_none_or(|buffer| buffer.size() < bytes.len() as u64)
    {
        *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (bytes.len() as u64).next_power_of_two(),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    if let Some(buffer) = slot {
        queue.write_buffer(buffer, 0, bytes);
    }
}

/// What opening a GPU needs before a window exists: the instance, the adapter, the device and its queue.
struct Opened {
    /// The wgpu instance the device was found through.
    instance: wgpu::Instance,
    /// The GPU adapter the device was opened on.
    adapter: wgpu::Adapter,
    /// The opened GPU device.
    device: wgpu::Device,
    /// The queue frames are submitted to.
    queue: wgpu::Queue,
}

/// A GPU being opened on another thread while the program does other work, handed to [`Renderer::from_opening`] once there is a window to draw into.
pub struct Opening(std::thread::JoinHandle<Option<Opened>>);

impl Opening {
    /// Starts finding a GPU and opening a device on it, without waiting for the result.
    pub fn start() -> Self {
        Self(std::thread::spawn(|| {
            pollster::block_on(async {
                let instance = Renderer::instance();
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        force_fallback_adapter: false,
                        compatible_surface: None,
                        ..Default::default()
                    })
                    .await
                    .ok()?;
                let (device, queue) = adapter
                    .request_device(&wgpu::DeviceDescriptor {
                        label: Some("pandemonium"),
                        ..Default::default()
                    })
                    .await
                    .ok()?;
                Some(Opened {
                    instance,
                    adapter,
                    device,
                    queue,
                })
            })
        }))
    }
}

impl Renderer {
    /// The wgpu instance every renderer is made through, over the primary graphics interfaces only so that no other driver is loaded.
    fn instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        })
    }

    /// Opens a renderer for a window on the GPU `opening` found while the program did other work, and configures its surface at the given physical size.
    ///
    /// When the opening failed, or its adapter cannot present to the window, the GPU is found again with the window's surface in mind, as [`Renderer::new`] does.
    pub fn from_opening(
        opening: Opening,
        target: impl Into<wgpu::SurfaceTarget<'static>> + Clone,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        if let Ok(Some(opened)) = opening.0.join() {
            let surface = opened
                .instance
                .create_surface(target.clone().into())
                .map_err(RendererError::Surface)?;
            if opened.adapter.is_surface_supported(&surface) {
                return Ok(Self::build(
                    opened.instance,
                    opened.adapter,
                    opened.device,
                    opened.queue,
                    Some(surface),
                    (width, height),
                    scale,
                ));
            }
        }
        Self::new(target, width, height, scale)
    }

    /// Opens a device for a window and configures its surface at the given
    /// physical size.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        pollster::block_on(Self::create(Some(target.into()), width, height, scale))
    }

    /// Opens a device with no window, drawing into a texture that
    /// [`Renderer::snapshot`] reads back.
    pub fn headless(width: u32, height: u32, scale: f32) -> Result<Self, RendererError> {
        pollster::block_on(Self::create(None, width, height, scale))
    }

    /// Opens a renderer for another window that draws with this renderer's
    /// device, so that every window of the application shares one GPU device.
    pub fn beside(
        &self,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        let surface = self
            .instance
            .create_surface(target.into())
            .map_err(RendererError::Surface)?;
        Ok(Self::build(
            self.instance.clone(),
            self.adapter.clone(),
            self.device.clone(),
            self.queue.clone(),
            Some(surface),
            (width, height),
            scale,
        ))
    }

    /// Acquires an adapter and device, then builds the target and the pipeline.
    async fn create(
        window: Option<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        let instance = Self::instance();
        let surface = window
            .map(|window| instance.create_surface(window))
            .transpose()
            .map_err(RendererError::Surface)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface.as_ref(),
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

        Ok(Self::build(
            instance,
            adapter,
            device,
            queue,
            surface,
            (width, height),
            scale,
        ))
    }

    /// Builds the target, a window's `surface` or a texture when there is
    /// none, `size` physical pixels across, and the pipeline on `device`.
    fn build(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        size: (u32, u32),
        scale: f32,
    ) -> Self {
        let (width, height) = size;
        let (target, format) = match surface {
            Some(surface) => {
                let capabilities = surface.get_capabilities(&adapter);
                let format = surface_format(&capabilities.formats);
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width: width.max(1),
                    height: height.max(1),
                    present_mode: wgpu::PresentMode::AutoVsync,
                    alpha_mode: [
                        wgpu::CompositeAlphaMode::PreMultiplied,
                        wgpu::CompositeAlphaMode::PostMultiplied,
                    ]
                    .into_iter()
                    .find(|mode| capabilities.alpha_modes.contains(mode))
                    .unwrap_or(capabilities.alpha_modes[0]),
                    color_space: wgpu::SurfaceColorSpace::Srgb,
                    view_formats: vec![],
                    desired_maximum_frame_latency: std::env::var("PANDEMONIUM_FRAME_LATENCY")
                        .ok()
                        .and_then(|value| value.parse::<u32>().ok())
                        .filter(|value| matches!(value, 1 | 2))
                        .unwrap_or(2),
                };
                surface.configure(&device, &config);
                let target = Target::Surface {
                    surface,
                    configured_size: (config.width, config.height),
                    config,
                };
                (target, format)
            }
            None => {
                let size = (width.max(1), height.max(1));
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("offscreen"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: OFFSCREEN_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                (Target::Offscreen { texture, size }, OFFSCREEN_FORMAT)
            }
        };

        let viewport_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let viewports = vec![viewport_uniform(&device, &viewport_layout)];
        let atlas = Atlas::new(&device);
        let quad_pipeline = build_pipeline(
            &device,
            format,
            "quad",
            pm_shaders::QUAD,
            &[Some(&viewport_layout)],
            size_of::<QuadInstance>() as u64,
            &QuadInstance::ATTRIBUTES,
        );
        let glyph_pipeline = build_pipeline(
            &device,
            format,
            "glyph",
            pm_shaders::GLYPH,
            &[Some(&viewport_layout), Some(atlas.layout())],
            size_of::<GlyphInstance>() as u64,
            &GlyphInstance::ATTRIBUTES,
        );
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("picture"),
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
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("picture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let image_pipeline = build_pipeline(
            &device,
            format,
            "picture",
            pm_shaders::IMAGE,
            &[Some(&viewport_layout), Some(&image_layout)],
            size_of::<GlyphInstance>() as u64,
            &GlyphInstance::ATTRIBUTES,
        );

        Self {
            instance,
            adapter,
            device,
            queue,
            target,
            scale: scale.max(f32::EPSILON),
            band: 0.0,
            viewport_layout,
            viewports,
            quad_pipeline,
            glyph_pipeline,
            image_pipeline,
            image_layout,
            image_sampler,
            images: HashMap::new(),
            live: HashMap::new(),
            image_instances: None,
            atlas,
            quad_instances: None,
            glyph_instances: None,
        }
    }

    /// Puts `picture` on the GPU unless a picture with its key already is there.
    fn upload_picture(&mut self, picture: &Picture) {
        if self.images.contains_key(&picture.key) || picture.width == 0 || picture.height == 0 {
            return;
        }
        if let Some(surface) = Picture::live_surface(picture.key) {
            self.upload_live(surface, picture);
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("picture"),
            size: wgpu::Extent3d {
                width: picture.width,
                height: picture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &picture.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(picture.width * 4),
                rows_per_image: Some(picture.height),
            },
            wgpu::Extent3d {
                width: picture.width,
                height: picture.height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("picture"),
            layout: &self.image_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.image_sampler),
                },
            ],
        });
        self.images.insert(picture.key, group);
    }

    /// Writes `picture`, a frame of live `surface`, over the frame before it, making the texture only when the size changes.
    fn upload_live(&mut self, surface: u32, picture: &Picture) {
        let size = (picture.width, picture.height);
        let extent = wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        if self.live.get(&surface).is_none_or(|live| live.size != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("live picture"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("live picture"),
                layout: &self.image_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.image_sampler),
                    },
                ],
            });
            if let Some(old) = self.live.insert(
                surface,
                Live {
                    texture,
                    size,
                    group,
                    key: picture.key,
                },
            ) {
                self.images.remove(&old.key);
            }
        }
        let Some(live) = self.live.get_mut(&surface) else {
            return;
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &live.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &picture.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(picture.width * 4),
                rows_per_image: Some(picture.height),
            },
            extent,
        );
        self.images.remove(&live.key);
        live.key = picture.key;
        self.images.insert(picture.key, live.group.clone());
    }

    /// The size of the drawing in logical pixels: the target without the band around it.
    pub fn size(&self) -> (f32, f32) {
        let (width, height) = self.physical_size();
        let band = 2.0 * self.offset();
        (
            (width as f32 - band).max(0.0) / self.scale,
            (height as f32 - band).max(0.0) / self.scale,
        )
    }

    /// Leaves a band `band` logical pixels wide around the drawing from the next frame on, for what spills out of it such as the shadow of a window. The drawing keeps its own coordinates, starting inside the band.
    pub fn set_band(&mut self, band: f32) {
        self.band = band.max(0.0);
    }

    /// How far right and down the drawing starts in the target, in whole physical pixels.
    fn offset(&self) -> f32 {
        (self.band * self.scale).round()
    }

    /// The target size in physical pixels.
    fn physical_size(&self) -> (u32, u32) {
        match &self.target {
            Target::Surface { config, .. } => (config.width, config.height),
            Target::Offscreen { size, .. } => *size,
        }
    }

    /// Records the latest physical size and scale factor for the next frame.
    ///
    /// Resize events can arrive faster than frames are presented, so the
    /// swapchain is recreated only when a frame needs the final size.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.scale = scale.max(f32::EPSILON);
        if let Target::Surface { config, .. } = &mut self.target {
            config.width = width.max(1);
            config.height = height.max(1);
        }
    }

    /// Recreates the swapchain at the latest requested physical size.
    fn configure_surface(&mut self) {
        if let Target::Surface {
            surface,
            config,
            configured_size,
        } = &mut self.target
        {
            surface.configure(&self.device, config);
            *configured_size = (config.width, config.height);
        }
    }

    /// Draws `list` over `ground` and presents it, notifying the window
    /// through `before_present` immediately before the frame goes to the
    /// display. Returns whether a frame was presented.
    ///
    /// A surface that timed out or is hidden skips the frame and is asked
    /// again on the next one; only a surface that is outdated or lost is
    /// configured again, along with a changed physical size.
    pub fn render(
        &mut self,
        list: &DrawList,
        shaper: &mut Shaper,
        ground: Srgba,
        before_present: impl FnOnce(),
    ) -> bool {
        if let Target::Surface {
            config,
            configured_size,
            ..
        } = &self.target
            && *configured_size != (config.width, config.height)
        {
            self.configure_surface();
        }
        let Target::Surface { surface, .. } = &self.target else {
            return false;
        };
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface();
                return false;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return false,
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.encode(&mut encoder, &view, list, shaper, ground);
        self.queue.submit(Some(encoder.finish()));
        before_present();
        self.queue.present(frame);
        true
    }

    /// Draws `list` over `ground` into the offscreen texture and returns its
    /// pixels as tightly packed RGBA bytes, row by row from the top.
    ///
    /// Returns nothing for a renderer that draws into a window.
    pub fn snapshot(
        &mut self,
        list: &DrawList,
        shaper: &mut Shaper,
        ground: Srgba,
    ) -> Option<Vec<u8>> {
        let Target::Offscreen { texture, size } = &self.target else {
            return None;
        };
        let (width, height) = *size;
        let texture = texture.clone();
        let row = width * 4;
        let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.encode(&mut encoder, &view, list, shaper, ground);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let mapped = slice.get_mapped_range().ok()?;
        let mut pixels = Vec::with_capacity((row * height) as usize);
        for line in mapped.chunks(padded as usize) {
            pixels.extend_from_slice(&line[..row as usize]);
        }
        drop(mapped);
        readback.unmap();
        Some(pixels)
    }

    /// Records the render pass that clears `view` to `ground` and draws `list`.
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        list: &DrawList,
        shaper: &mut Shaper,
        ground: Srgba,
    ) {
        let (width, height) = self.physical_size();
        let mut quads: Vec<QuadInstance> = Vec::new();
        let mut glyphs: Vec<GlyphInstance> = Vec::new();
        let mut pictures: Vec<GlyphInstance> = Vec::new();
        let mut batches: Vec<Batch> = Vec::new();
        let mut clip: Option<Rect> = None;
        let mut masks: Vec<Mask> = Vec::new();
        let mut viewport = 0;
        for primitive in list.primitives() {
            match primitive {
                Primitive::Quad(quad) => {
                    let at = quads.len() as u32;
                    quads.push(QuadInstance::new(quad));
                    extend(&mut batches, Kind::Quad, at..at + 1, (clip, viewport));
                }
                Primitive::Clip(next) => clip = *next,
                Primitive::Mask(next) => {
                    viewport = match next {
                        Some(mask) => {
                            let known = masks.iter().position(|held| held == mask);
                            let index = known.unwrap_or_else(|| {
                                masks.push(*mask);
                                masks.len() - 1
                            });
                            index + 1
                        }
                        None => 0,
                    };
                }
                Primitive::Image(image) => {
                    let picture = &image.picture;
                    self.upload_picture(picture);
                    if !self.images.contains_key(&picture.key) {
                        continue;
                    }
                    let (width, height) = (picture.width as f32, picture.height as f32);
                    let wanted = image.width / image.height.max(f32::EPSILON);
                    let held = width / height;
                    let (across, down) = if held > wanted {
                        (wanted / held, 1.0)
                    } else {
                        (1.0, held / wanted)
                    };
                    let at = pictures.len() as u32;
                    pictures.push(GlyphInstance {
                        origin: [image.x * self.scale, image.y * self.scale],
                        size: [image.width * self.scale, image.height * self.scale],
                        uv: [
                            (1.0 - across) / 2.0,
                            (1.0 - down) / 2.0,
                            (1.0 + across) / 2.0,
                            (1.0 + down) / 2.0,
                        ],
                        color: [image.radius * self.scale, 0.0, 0.0, 1.0],
                        rotation: 0.0,
                    });
                    extend(
                        &mut batches,
                        Kind::Image(picture.key),
                        at..at + 1,
                        (clip, viewport),
                    );
                }
                Primitive::Icon(icon) => {
                    let pixels = (icon.size * self.scale).round() as u32;
                    let Some(entry) = self.atlas.icon(&self.queue, icon.svg, pixels) else {
                        continue;
                    };
                    let at = glyphs.len() as u32;
                    glyphs.push(GlyphInstance {
                        origin: [(icon.x * self.scale).round(), (icon.y * self.scale).round()],
                        size: [entry.width, entry.height],
                        uv: entry.uv,
                        color: icon.color.to_array(),
                        rotation: icon.rotation,
                    });
                    extend(&mut batches, Kind::Glyph, at..at + 1, (clip, viewport));
                }
                Primitive::Text(text) => {
                    let start = glyphs.len() as u32;
                    for placed in text.run.place(text.x, text.y, self.scale) {
                        let Some(entry) = self.atlas.entry(&self.queue, shaper, placed.key) else {
                            continue;
                        };
                        glyphs.push(GlyphInstance {
                            origin: [placed.x as f32 + entry.left, placed.y as f32 - entry.top],
                            size: [entry.width, entry.height],
                            uv: entry.uv,
                            color: text.color.to_array(),
                            rotation: 0.0,
                        });
                    }
                    let end = glyphs.len() as u32;
                    if end > start {
                        extend(&mut batches, Kind::Glyph, start..end, (clip, viewport));
                    }
                }
            }
        }
        upload(
            &self.device,
            &self.queue,
            &mut self.quad_instances,
            "quads",
            bytemuck::cast_slice(&quads),
        );
        upload(
            &self.device,
            &self.queue,
            &mut self.glyph_instances,
            "glyphs",
            bytemuck::cast_slice(&glyphs),
        );
        upload(
            &self.device,
            &self.queue,
            &mut self.image_instances,
            "pictures",
            bytemuck::cast_slice(&pictures),
        );

        let offset = self.offset();
        while self.viewports.len() <= masks.len() {
            self.viewports
                .push(viewport_uniform(&self.device, &self.viewport_layout));
        }
        let masked = std::iter::once(None).chain(masks.into_iter().map(Some));
        for ((buffer, _), mask) in self.viewports.iter().zip(masked) {
            self.queue.write_buffer(
                buffer,
                0,
                bytemuck::bytes_of(&Viewport::new((width, height), self.scale, offset, mask)),
            );
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(ground.r),
                        g: f64::from(ground.g),
                        b: f64::from(ground.b),
                        a: f64::from(ground.a),
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        for Batch {
            kind,
            range,
            clip,
            viewport,
        } in batches
        {
            let Some((x, y, w, h)) = scissor(clip, self.scale, offset, (width, height)) else {
                continue;
            };
            pass.set_scissor_rect(x, y, w, h);
            pass.set_bind_group(0, &self.viewports[viewport].1, &[]);
            if let Kind::Image(key) = kind {
                if let (Some(group), Some(buffer)) = (self.images.get(&key), &self.image_instances)
                {
                    pass.set_pipeline(&self.image_pipeline);
                    pass.set_bind_group(1, group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, range);
                }
                continue;
            }
            match (kind, &self.quad_instances, &self.glyph_instances) {
                (Kind::Quad, Some(buffer), _) => {
                    pass.set_pipeline(&self.quad_pipeline);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, range);
                }
                (Kind::Glyph, _, Some(buffer)) => {
                    pass.set_pipeline(&self.glyph_pipeline);
                    pass.set_bind_group(1, self.atlas.group(), &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, range);
                }
                _ => {}
            }
        }
    }
}

/// The format a window is drawn in, of the `offered` ones: one with eight bits of opacity, which a soft shadow over what is behind the window needs, before any other that is not sRGB-encoded, before the first offered.
fn surface_format(offered: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ]
    .into_iter()
    .find(|format| offered.contains(format))
    .or_else(|| offered.iter().copied().find(|format| !format.is_srgb()))
    .unwrap_or(offered[0])
}

/// A run of instances of one kind drawn together.
struct Batch {
    /// What the instances are.
    kind: Kind,
    /// Which instances of that kind's buffer are drawn.
    range: Range<u32>,
    /// The rectangle drawing is kept inside, when there is one.
    clip: Option<Rect>,
    /// Which viewport of the frame the instances are drawn with: the first masks nothing.
    viewport: usize,
}

/// Adds `range` to the last batch when it is of the same `kind`, runs on
/// from it and is limited the same way, by the clip and the viewport of
/// `limits`, and starts a new batch otherwise.
fn extend(batches: &mut Vec<Batch>, kind: Kind, range: Range<u32>, limits: (Option<Rect>, usize)) {
    let (clip, viewport) = limits;
    match batches.last_mut() {
        Some(last)
            if last.kind == kind
                && last.range.end == range.start
                && last.clip == clip
                && last.viewport == viewport =>
        {
            last.range.end = range.end;
        }
        _ => batches.push(Batch {
            kind,
            range,
            clip,
            viewport,
        }),
    }
}

/// A uniform that holds one viewport, and the bind group of `layout` that exposes it to the shaders.
fn viewport_uniform(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("viewport"),
        size: size_of::<Viewport>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, group)
}

/// The scissor rectangle, in physical pixels inside a target of `size`, that
/// keeps drawing inside `clip`, which is drawn `offset` physical pixels right
/// and down of where it says it is; the whole target without one, and
/// nothing when the clip leaves no pixel of it.
fn scissor(
    clip: Option<Rect>,
    scale: f32,
    offset: f32,
    size: (u32, u32),
) -> Option<(u32, u32, u32, u32)> {
    let Some(clip) = clip else {
        return Some((0, 0, size.0, size.1));
    };
    let left = (clip.x * scale + offset).floor().clamp(0.0, size.0 as f32) as u32;
    let top = (clip.y * scale + offset).floor().clamp(0.0, size.1 as f32) as u32;
    let right = ((clip.x + clip.width) * scale + offset)
        .ceil()
        .clamp(0.0, size.0 as f32) as u32;
    let bottom = ((clip.y + clip.height) * scale + offset)
        .ceil()
        .clamp(0.0, size.1 as f32) as u32;
    (right > left && bottom > top).then_some((left, top, right - left, bottom - top))
}

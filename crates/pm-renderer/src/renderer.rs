//! The device, the surface and the one frame each redraw presents.

use pm_color::Srgba;

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
    /// The window's swapchain surface.
    surface: wgpu::Surface<'static>,
    /// The opened GPU device.
    device: wgpu::Device,
    /// The queue frames are submitted to.
    queue: wgpu::Queue,
    /// How the surface is configured at the latest requested size.
    config: wgpu::SurfaceConfiguration,
    /// The physical size the swapchain was last configured to present.
    configured_size: (u32, u32),
}

impl Renderer {
    /// Opens a device for `target` and configures its surface at the given
    /// physical size.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, RendererError> {
        pollster::block_on(Self::create(target, width, height))
    }

    /// Acquires an adapter and device, then configures the surface.
    async fn create(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
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

        Ok(Self {
            surface,
            device,
            queue,
            configured_size: (config.width, config.height),
            config,
        })
    }

    /// Records the latest physical size for the next frame.
    ///
    /// Resize events can arrive faster than frames are presented, so the
    /// swapchain is recreated only when a frame needs the final size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
    }

    /// Recreates the swapchain at the latest requested physical size.
    fn configure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
        self.configured_size = (self.config.width, self.config.height);
    }

    /// Clears the surface to `ground` and presents it, notifying the window
    /// through `before_present` immediately before the frame goes to the
    /// display.
    ///
    /// A surface that timed out or is hidden skips the frame and is asked
    /// again on the next one; only a surface that is outdated or lost is
    /// configured again, along with a changed physical size.
    pub fn render(&mut self, ground: Srgba, before_present: impl FnOnce()) {
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

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
        }

        self.queue.submit(Some(encoder.finish()));
        before_present();
        self.queue.present(frame);
    }
}

//! The instance layouts the shaders read, and the pipelines that draw them.
//!
//! Every pipeline is the same shape: six vertices a quad, one instance per
//! thing drawn, one uniform holding the viewport. Adding another is adding an
//! instance struct and a shader here, never a render pass in a caller.

/// The viewport size every shader turns pixels into clip space with.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Viewport {
    /// Surface size in physical pixels.
    pub(crate) size: [f32; 2],
    /// Padding to the 16-byte alignment a uniform block needs.
    pub(crate) padding: [f32; 2],
}

/// One rounded rectangle, in physical pixels, as the quad pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct QuadInstance {
    /// Top-left corner.
    pub(crate) origin: [f32; 2],
    /// Extent from the origin.
    pub(crate) size: [f32; 2],
    /// Fill colour, sRGB-encoded.
    pub(crate) background: [f32; 4],
    /// Border colour, sRGB-encoded.
    pub(crate) border_color: [f32; 4],
    /// Corner radii, clockwise from the top-left corner.
    pub(crate) radii: [f32; 4],
    /// Border width, and padding to the next attribute.
    pub(crate) border: [f32; 2],
    /// Clip rectangle as left, top, right, bottom.
    pub(crate) clip: [f32; 4],
}

impl QuadInstance {
    /// The vertex attributes the quad shader expects, in declaration order.
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x2,
        6 => Float32x4,
    ];
}

/// One glyph, in physical pixels, as the glyph pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GlyphInstance {
    /// Top-left corner of the glyph's bitmap.
    pub(crate) origin: [f32; 2],
    /// Size of the glyph's bitmap.
    pub(crate) size: [f32; 2],
    /// Top-left corner in atlas texture coordinates.
    pub(crate) uv_origin: [f32; 2],
    /// Extent in atlas texture coordinates.
    pub(crate) uv_size: [f32; 2],
    /// Tint, sRGB-encoded.
    pub(crate) color: [f32; 4],
    /// Clip rectangle as left, top, right, bottom.
    pub(crate) clip: [f32; 4],
    /// Rotation in radians, followed by alignment padding.
    pub(crate) rotation: [f32; 4],
}

impl GlyphInstance {
    /// The vertex attributes the glyph shader expects, in declaration order.
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
    ];
}

/// One picture, in physical pixels, as the image pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ImageInstance {
    /// Top-left corner of the rectangle the picture is stretched over.
    pub(crate) origin: [f32; 2],
    /// Extent of that rectangle.
    pub(crate) size: [f32; 2],
    /// Clip rectangle as left, top, right, bottom.
    pub(crate) clip: [f32; 4],
}

impl ImageInstance {
    /// The vertex attributes the image shader expects, in declaration order.
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x4,
    ];
}

/// Vertex buffers split at instance boundaries to respect the device's size limit.
pub(crate) struct InstanceBuffer {
    /// Debug label carried onto every allocation.
    label: &'static str,
    /// Reusable buffers, one per uploaded chunk.
    buffers: Vec<wgpu::Buffer>,
    /// Maximum number of instances in each chunk.
    chunk_len: usize,
}

impl InstanceBuffer {
    /// Creates empty buffers that will be labelled `label`.
    pub(crate) fn new(label: &'static str) -> Self {
        Self {
            label,
            buffers: Vec::new(),
            chunk_len: 1,
        }
    }

    /// Uploads instances in chunks bounded by the device's maximum buffer size.
    pub(crate) fn upload<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[T],
    ) {
        let limit = device.limits().max_buffer_size;
        self.chunk_len = (limit / std::mem::size_of::<T>() as u64) as usize;
        let chunks = instances.len().div_ceil(self.chunk_len);
        self.buffers.truncate(chunks);
        for (index, chunk) in instances.chunks(self.chunk_len).enumerate() {
            let data = bytemuck::cast_slice(chunk);
            if self
                .buffers
                .get(index)
                .is_none_or(|buffer| buffer.size() < data.len() as u64)
            {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(self.label),
                    size: (data.len() as u64).next_power_of_two().min(limit),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                if index < self.buffers.len() {
                    self.buffers[index] = buffer;
                } else {
                    self.buffers.push(buffer);
                }
            }
            queue.write_buffer(&self.buffers[index], 0, data);
        }
    }

    /// Visits the buffers intersecting a global instance range, with local draw ranges.
    pub(crate) fn slices(
        &self,
        range: std::ops::Range<u32>,
    ) -> impl Iterator<Item = (&wgpu::Buffer, std::ops::Range<u32>, usize)> {
        let chunk_len = self.chunk_len;
        let start = range.start as usize;
        let end = range.end as usize;
        (start / chunk_len..end.div_ceil(chunk_len)).filter_map(move |index| {
            let base = index * chunk_len;
            let local = (start.saturating_sub(base)) as u32..(end - base).min(chunk_len) as u32;
            self.buffers.get(index).map(|buffer| (buffer, local, base))
        })
    }
}

/// Builds one instanced pipeline from a shader that draws six vertices a quad.
pub(crate) fn build_pipeline(
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

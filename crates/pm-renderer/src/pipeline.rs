//! The instance layout the quad shader reads, and the pipeline that draws it.

use crate::draw::Quad;

/// The viewport every shader turns logical pixels into clip space with.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Viewport {
    /// Target size in physical pixels.
    pub(crate) size: [f32; 2],
    /// Physical pixels per logical pixel.
    pub(crate) scale: f32,
    /// Padding to the 16-byte alignment a uniform block needs.
    pub(crate) padding: f32,
}

/// One quad as the shader reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct QuadInstance {
    /// Left, top, width and height in logical pixels.
    rect: [f32; 4],
    /// Fill colour.
    fill: [f32; 4],
    /// Border colour.
    border_color: [f32; 4],
    /// Corner radii, clockwise from the top-left.
    radii: [f32; 4],
    /// The transform's `a`, `b`, `c` and `d`.
    matrix: [f32; 4],
    /// The transform's `tx` and `ty`, the border width and the softness.
    params: [f32; 4],
    /// The arc's sweep and ring width; a width of nought is a rectangle.
    arc: [f32; 4],
}

impl QuadInstance {
    /// The vertex attributes the quad shader expects, in declaration order.
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
    ];

    /// The instance that draws `quad`.
    pub(crate) fn new(quad: &Quad) -> Self {
        let transform = quad.transform;
        Self {
            rect: [
                quad.bounds.x,
                quad.bounds.y,
                quad.bounds.width,
                quad.bounds.height,
            ],
            fill: quad.fill.to_array(),
            border_color: quad.border_color.to_array(),
            radii: quad.radii,
            matrix: [transform.a, transform.b, transform.c, transform.d],
            params: [transform.tx, transform.ty, quad.border_width, quad.softness],
            arc: [quad.arc_sweep, quad.arc_width, 0.0, 0.0],
        }
    }
}

/// One glyph as the shader reads it, in physical pixels.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GlyphInstance {
    /// Top-left corner of the glyph's bitmap.
    pub(crate) origin: [f32; 2],
    /// Size of the glyph's bitmap.
    pub(crate) size: [f32; 2],
    /// Left, top, right and bottom in atlas texture coordinates.
    pub(crate) uv: [f32; 4],
    /// Tint, sRGB-encoded with straight alpha.
    pub(crate) color: [f32; 4],
    /// How far the glyph is turned clockwise about the middle of its bitmap, in radians.
    pub(crate) rotation: f32,
}

impl GlyphInstance {
    /// The vertex attributes the glyph shader expects, in declaration order.
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32,
    ];
}

/// Builds a pipeline that draws instanced quads of six vertices each from
/// `source`, reading instances of `stride` bytes laid out as `attributes`.
pub(crate) fn build_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    label: &'static str,
    source: &str,
    bind_group_layouts: &[Option<&wgpu::BindGroupLayout>],
    stride: u64,
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
    let premultiplied = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: stride,
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
                    color: premultiplied,
                    alpha: premultiplied,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

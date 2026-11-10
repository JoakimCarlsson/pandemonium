// Pictures: one instanced quad each, sampling a full-colour texture with rounded corners.

struct Viewport {
    size: vec2<f32>,
    scale: f32,
    mask_radius: f32,
    mask: vec4<f32>,
    offset: vec2<f32>,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var picture_sampler: sampler;

struct Instance {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) half_size: vec2<f32>,
    @location(3) radius: f32,
}

// How much of the pixel at `position`, in physical pixels, lies inside the
// rounded rectangle everything is masked to.
fn mask_coverage(position: vec2<f32>) -> f32 {
    let half_size = (viewport.mask.zw - viewport.mask.xy) * 0.5;
    let centre = (viewport.mask.xy + viewport.mask.zw) * 0.5;
    let inner = abs(position - centre) - half_size + vec2<f32>(viewport.mask_radius);
    let distance = length(max(inner, vec2<f32>(0.0))) + min(max(inner.x, inner.y), 0.0)
        - viewport.mask_radius;
    return clamp(0.5 - distance, 0.0, 1.0);
}

fn unit_corner(index: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    return corners[index];
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, instance: Instance) -> Fragment {
    let corner = unit_corner(index);
    let point = viewport.offset + instance.origin + corner * instance.size;

    var out: Fragment;
    out.position = vec4<f32>(
        point.x / viewport.size.x * 2.0 - 1.0,
        1.0 - point.y / viewport.size.y * 2.0,
        0.0,
        1.0,
    );
    out.uv = mix(instance.uv.xy, instance.uv.zw, corner);
    out.half_size = instance.size / 2.0;
    out.local = (corner - vec2<f32>(0.5, 0.5)) * instance.size;
    out.radius = instance.color.x;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    let pixel = textureSample(picture, picture_sampler, in.uv);
    let inner = in.half_size - vec2<f32>(in.radius, in.radius);
    let offset = abs(in.local) - inner;
    let distance = length(max(offset, vec2<f32>(0.0, 0.0))) + min(max(offset.x, offset.y), 0.0) - in.radius;
    let coverage = clamp(0.5 - distance, 0.0, 1.0);
    let alpha = pixel.a * coverage * mask_coverage(in.position.xy);
    return vec4<f32>(pixel.rgb * alpha, alpha);
}

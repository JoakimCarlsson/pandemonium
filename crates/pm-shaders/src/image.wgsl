// Pictures: one instanced quad each, sampling a full-colour texture with rounded corners.

struct Viewport {
    size: vec2<f32>,
    scale: f32,
    padding: f32,
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
    let point = instance.origin + corner * instance.size;

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
    let alpha = pixel.a * coverage;
    return vec4<f32>(pixel.rgb * alpha, alpha);
}

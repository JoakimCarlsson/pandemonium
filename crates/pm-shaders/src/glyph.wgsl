// Glyphs: one instanced quad each, sampling a coverage atlas and tinting it.

struct Viewport {
    size: vec2<f32>,
    scale: f32,
    padding: f32,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct Instance {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) rotation: f32,
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
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
    let middle = instance.origin + instance.size * 0.5;
    let offset = (corner - vec2<f32>(0.5, 0.5)) * instance.size;
    let sine = sin(instance.rotation);
    let cosine = cos(instance.rotation);
    let point = middle + vec2<f32>(
        offset.x * cosine - offset.y * sine,
        offset.x * sine + offset.y * cosine,
    );

    var out: Fragment;
    out.position = vec4<f32>(
        point.x / viewport.size.x * 2.0 - 1.0,
        1.0 - point.y / viewport.size.y * 2.0,
        0.0,
        1.0,
    );
    out.uv = mix(instance.uv.xy, instance.uv.zw, corner);
    out.color = instance.color;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas, atlas_sampler, in.uv).r;
    let alpha = in.color.a * coverage;
    return vec4<f32>(in.color.rgb * alpha, alpha);
}

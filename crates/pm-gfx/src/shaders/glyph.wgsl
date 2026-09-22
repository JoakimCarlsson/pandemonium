// Glyphs: one instanced quad per glyph, textured by its coverage mask in the
// shared atlas and tinted by the run's colour.

struct Viewport {
    size: vec2<f32>,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct Instance {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_origin: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) clip: vec4<f32>,
    @location(6) rotation: vec4<f32>,
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) clip: vec4<f32>,
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
    let local = (corner - vec2<f32>(0.5, 0.5)) * instance.size;
    let sine = sin(instance.rotation.x);
    let cosine = cos(instance.rotation.x);
    let rotated = vec2<f32>(
        local.x * cosine - local.y * sine,
        local.x * sine + local.y * cosine,
    );
    let point = instance.origin + instance.size * 0.5 + rotated;

    var out: Fragment;
    out.position = vec4<f32>(
        point.x / viewport.size.x * 2.0 - 1.0,
        1.0 - point.y / viewport.size.y * 2.0,
        0.0,
        1.0,
    );
    out.uv = instance.uv_origin + corner * instance.uv_size;
    out.color = instance.color;
    out.clip = instance.clip;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    if in.position.x < in.clip.x || in.position.x > in.clip.z
        || in.position.y < in.clip.y || in.position.y > in.clip.w {
        discard;
    }

    let coverage = textureSample(atlas, atlas_sampler, in.uv).r;
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a) * coverage;
}

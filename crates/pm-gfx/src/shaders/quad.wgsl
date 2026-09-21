// Rounded, bordered rectangles: one instanced quad each, shaped by a signed
// distance field so corners and borders antialias without geometry.

struct Viewport {
    size: vec2<f32>,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;

struct Instance {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) shape: vec2<f32>,
    @location(5) clip: vec4<f32>,
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) shape: vec2<f32>,
    @location(5) clip: vec4<f32>,
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

fn rounded_box(point: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let inner = abs(point) - half_size + vec2<f32>(radius);
    return length(max(inner, vec2<f32>(0.0))) + min(max(inner.x, inner.y), 0.0) - radius;
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
    out.local = (corner - vec2<f32>(0.5)) * instance.size;
    out.half_size = instance.size * 0.5;
    out.background = instance.background;
    out.border_color = instance.border_color;
    out.shape = instance.shape;
    out.clip = instance.clip;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    if in.position.x < in.clip.x || in.position.x > in.clip.z
        || in.position.y < in.clip.y || in.position.y > in.clip.w {
        discard;
    }

    let radius = min(in.shape.x, min(in.half_size.x, in.half_size.y));
    let coverage = 1.0 - smoothstep(-0.5, 0.5, rounded_box(in.local, in.half_size, radius));
    let background = vec4<f32>(in.background.rgb * in.background.a, in.background.a);

    var color = background;
    let border = in.shape.y;
    if border > 0.0 {
        let inner_half = max(in.half_size - vec2<f32>(border), vec2<f32>(0.0));
        let inner_radius = max(radius - border, 0.0);
        let inner = 1.0 - smoothstep(-0.5, 0.5, rounded_box(in.local, inner_half, inner_radius));
        let stroke = vec4<f32>(in.border_color.rgb * in.border_color.a, in.border_color.a);
        color = mix(stroke, background, inner);
    }

    return color * coverage;
}

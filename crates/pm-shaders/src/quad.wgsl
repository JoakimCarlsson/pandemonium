// Rounded, bordered, optionally blurred rectangles: one instanced quad each,
// shaped by a signed distance field so corners, borders and soft edges
// antialias without geometry.

struct Viewport {
    size: vec2<f32>,
    scale: f32,
    padding: f32,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) fill: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) radii: vec4<f32>,
    @location(4) matrix: vec4<f32>,
    @location(5) params: vec4<f32>,
    @location(6) arc: vec4<f32>,
}

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) fill: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) radii: vec4<f32>,
    @location(5) params: vec4<f32>,
    @location(6) arc: vec4<f32>,
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

// The radius of the corner `point` lies in, with the radii running clockwise
// from the top-left.
fn corner_radius(radii: vec4<f32>, point: vec2<f32>) -> f32 {
    let top = select(radii.x, radii.y, point.x > 0.0);
    let bottom = select(radii.w, radii.z, point.x > 0.0);
    return select(top, bottom, point.y > 0.0);
}

fn rounded_box(point: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let inner = abs(point) - half_size + vec2<f32>(radius);
    return length(max(inner, vec2<f32>(0.0))) + min(max(inner.x, inner.y), 0.0) - radius;
}

// The distance from `point` to a ring of radius `ring` and thickness
// `thickness` that covers `sweep` of the way round, clockwise from the top,
// with round ends: negative inside.
fn arc_distance(point: vec2<f32>, ring: f32, thickness: f32, sweep: f32) -> f32 {
    let half_angle = sweep * 3.14159265;
    let middle = vec2<f32>(sin(half_angle), -cos(half_angle));
    let across = vec2<f32>(-middle.y, middle.x);
    var turned = vec2<f32>(dot(point, across), dot(point, middle));
    turned.x = abs(turned.x);
    let ends = vec2<f32>(sin(half_angle), cos(half_angle));
    let to_ring = select(
        abs(length(turned) - ring),
        length(turned - ends * ring),
        ends.y * turned.x > ends.x * turned.y,
    );
    return to_ring - thickness * 0.5;
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, instance: Instance) -> Fragment {
    let softness = instance.params.w;
    let margin = max(softness * 2.0, 1.0);
    let size = instance.rect.zw;
    let corner = unit_corner(index);
    let local = (corner - vec2<f32>(0.5)) * (size + vec2<f32>(margin * 2.0));
    let centre = instance.rect.xy + size * 0.5;
    let point = centre + local;
    let moved = vec2<f32>(
        instance.matrix.x * point.x + instance.matrix.z * point.y + instance.params.x,
        instance.matrix.y * point.x + instance.matrix.w * point.y + instance.params.y,
    );
    let physical = moved * viewport.scale;

    var out: Fragment;
    out.position = vec4<f32>(
        physical.x / viewport.size.x * 2.0 - 1.0,
        1.0 - physical.y / viewport.size.y * 2.0,
        0.0,
        1.0,
    );
    out.local = local;
    out.half_size = size * 0.5;
    out.fill = instance.fill;
    out.border_color = instance.border_color;
    out.radii = instance.radii;
    out.params = instance.params;
    out.arc = instance.arc;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    if in.arc.y > 0.0 {
        let ring = min(in.half_size.x, in.half_size.y) - in.arc.y * 0.5;
        let distance = arc_distance(in.local, ring, in.arc.y, in.arc.x);
        let edge = 0.5 / viewport.scale;
        let coverage = 1.0 - smoothstep(-edge, edge, distance);
        return vec4<f32>(in.fill.rgb * in.fill.a, in.fill.a) * coverage;
    }
    let limit = min(in.half_size.x, in.half_size.y);
    let radius = min(corner_radius(in.radii, in.local), limit);
    let distance = rounded_box(in.local, in.half_size, radius);
    let edge = 0.5 / viewport.scale;
    let softness = in.params.w;
    var coverage = 1.0 - smoothstep(-edge, edge, distance);
    if softness > 0.0 {
        coverage = 1.0 - smoothstep(-softness, softness, distance);
    }

    let fill = vec4<f32>(in.fill.rgb * in.fill.a, in.fill.a);
    let border = vec4<f32>(in.border_color.rgb * in.border_color.a, in.border_color.a);
    var colour = fill;
    let border_width = in.params.z;
    if border_width > 0.0 {
        let inside = 1.0 - smoothstep(-edge, edge, distance + border_width);
        colour = mix(border, fill, inside);
    }
    return colour * coverage;
}

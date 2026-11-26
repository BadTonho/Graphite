struct Globals {
    screen: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(0) @binding(3) var images: texture_2d<f32>;
@group(0) @binding(4) var image_sampler: sampler;

// One instance per rectangle: its corners and texture corners, color, and mode.
struct VsIn {
    @builtin(vertex_index) corner: u32,
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) mode: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) mode: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // Two triangles: top-left, top-right, bottom-left, then top-right, bottom-right, bottom-left.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let corner = corners[in.corner];
    let pos = mix(in.rect.xy, in.rect.zw, corner);
    var out: VsOut;
    let ndc = vec2<f32>(
        pos.x / globals.screen.x * 2.0 - 1.0,
        1.0 - pos.y / globals.screen.y * 2.0,
    );
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(in.uv.xy, in.uv.zw, corner);
    out.color = in.color;
    out.mode = in.mode;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Sampling needs uniform control flow, so both textures are read before choosing.
    let coverage = textureSample(atlas, atlas_sampler, in.uv).r;
    let texel = textureSample(images, image_sampler, in.uv);
    if (in.mode > 1.5) {
        // A tinted image mask: vertex color supplies RGB, texel alpha supplies coverage.
        return vec4<f32>(in.color.rgb, texel.a * in.color.a);
    }
    if (in.mode > 0.5) {
        // An image: the atlas holds its color; the vertex color only carries opacity.
        return vec4<f32>(texel.rgb, texel.a * in.color.a);
    }
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}

// Scale: nearest-neighbor sampling using a scale factor.
// Mode-aware: for L/LA (0/1) only copies R channel from source position.
// CPU reference (ops/pil_resize.rs): PIL pixel-centered coordinates.
// Maps output position (dx, dy) using the actual rounded output dimensions,
// matching pil_resize's source/output ratio. The requested scale factor is
// only used by the host to derive dst_w/dst_h; using 1/factor here diverges
// whenever the rounded dimensions are not exactly factor*source_dimensions.
//
struct Params {
    width: u32,
    height: u32,
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor_numer: u32,  // factor as fixed-point: round(factor * 65536)
    dst_w: u32,
    dst_h: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn scale_pixel(dx: u32, dy: u32) -> u32 {
    let src_w = params.width;
    let src_h = params.height;
    // PIL's nearest resize maps pixel centers using src/dst, then floors the
    // result. Do not derive this from the requested factor after rounding
    // the output dimensions.
    let cx = (f32(dx) + 0.5) * f32(src_w) / f32(params.dst_w);
    let cy = (f32(dy) + 0.5) * f32(src_h) / f32(params.dst_h);

    let sx = u32(clamp(floor(cx), 0.0, f32(src_w - 1u)));
    let sy = u32(clamp(floor(cy), 0.0, f32(src_h - 1u)));

    let pixel = input[sy * src_w + sx];
    let src_r = pixel & 0xffu;
    let src_g = (pixel >> 8u) & 0xffu;
    let src_b = (pixel >> 16u) & 0xffu;
    let src_a = (pixel >> 24u) & 0xffu;

    // Mode-aware: for L/LA, only copy R; zero G/B, A=255 for non-alpha modes
    let out_r = src_r;
    let out_g = select(0u, src_g, mode_has_g(params.mode));
    let out_b = select(0u, src_b, mode_has_b(params.mode));
    let out_a = select(255u, src_a, mode_has_a(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }
    let idx = gid.y * params.dst_w + gid.x;
    output[idx] = scale_pixel(gid.x, gid.y);
}

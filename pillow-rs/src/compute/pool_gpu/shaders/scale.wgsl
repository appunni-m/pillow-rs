// Scale: nearest-neighbor sampling using a scale factor.
// Mode-aware: for L/LA (0/1) only copies R channel from source position.
// CPU reference (ops/pil_resize.rs): PIL pixel-centered coordinates.
// Maps output position (dx, dy) to source (dx/factor, dy/factor).
//
// NOTE: The dispatch is based on source dimensions. For output resize
// support, the backend must be updated to handle different output sizes.

struct Params {
    width: u32,
    height: u32,
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor_numer: u32,  // factor as fixed-point: round(factor * 65536)
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn scale_pixel(dx: u32, dy: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let factor = f32(params.factor_numer) / 65536.0;

    // Map output pixel back to source: sx = dx / factor
    let inv_factor = 1.0 / factor;
    let cx = (f32(dx) + 0.5) * inv_factor;
    let cy = (f32(dy) + 0.5) * inv_factor;

    let sx = u32(clamp(floor(cx), 0.0, f32(w - 1u)));
    let sy = u32(clamp(floor(cy), 0.0, f32(h - 1u)));

    let pixel = input[sy * w + sx];
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
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    output[idx] = scale_pixel(gid.x, gid.y);
}

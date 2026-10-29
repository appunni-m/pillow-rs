// Contrast uses Pillow's fused float32 blend with the rounded image mean.
// The mean is computed by host control; pixel interpolation runs on the GPU.
// Mode-aware: only processes channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor: f32,
    midpoint: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn contrast_apply(c: u32, midpoint: u32, factor: f32) -> u32 {
    return u32(clamp(fma(factor, f32(c) - f32(midpoint), f32(midpoint)), 0.0, 255.0));
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let fi = params.factor;
    let is_cmyk = params.mode == 4u;
    let base_r = select(params.midpoint, 0u, is_cmyk);
    let base_g = select(params.midpoint, 0u, is_cmyk);
    let base_b = select(params.midpoint, 0u, is_cmyk);
    let base_a = select(255u, 255u - params.midpoint, is_cmyk);
    let val_r = contrast_apply(r, base_r, fi);
    let val_g = contrast_apply(g, base_g, fi);
    let val_b = contrast_apply(b, base_b, fi);
    let val_a = contrast_apply(a, base_a, fi);

    let out_r = val_r;
    let out_g = select(g, val_g, mode_has_g(params.mode) || is_cmyk);
    let out_b = select(b, val_b, mode_has_b(params.mode) || is_cmyk);
    let preserved_a = select(255u, a, mode_has_a(params.mode));
    let out_a = select(preserved_a, val_a, is_cmyk);

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

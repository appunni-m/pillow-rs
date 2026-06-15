// PointOp: per-channel lookup table for Image.point()
// out[c] = lut[in[c]]
// Params buffer holds 256-entry LUT as packed RGBA u32 values (1024 bytes)
//
// Mode-aware: L/LA only apply LUT to R channel; RGB apply to R,G,B; RGBA apply to R,G,B.
// Alpha is always preserved unchanged.
// Params: standard header, LUT array follows at [4].

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    lut: array<u32, 256>,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

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

    // Look up each channel in the packed RGBA LUT
    let out_r = params.lut[r] & 0xffu;
    let out_g = select(g, (params.lut[g] >> 8u) & 0xffu, mode_has_g(params.mode));
    let out_b = select(b, (params.lut[b] >> 16u) & 0xffu, mode_has_b(params.mode));
    // Alpha preserved unchanged

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (a << 24u);
}

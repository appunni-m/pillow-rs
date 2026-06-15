// PointOp: per-channel lookup table for Image.point()
// out[c] = lut[in[c]]
// LUT is a separate storage buffer (array<u32, 256>).
// Each LUT entry is packed RGBA: R | (G<<8) | (B<<16) | (A<<24)
// For each input pixel channel value c, the output channel value
// is extracted from the corresponding byte of the LUT entry at index c.
//
// 4-binding layout: input, output, params, lut.
// Same semantics as eval.wgsl. Separate shader for clarity.
//
// Mode-aware: L/LA only apply LUT to R channel; RGB apply to R,G,B; RGBA apply to R,G,B.
// Alpha is always preserved unchanged.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> lut: array<u32, 256>;

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
    // LUT entry byte layout: [R, G, B, A] at byte offsets 0, 1, 2, 3
    let out_r = lut[r] & 0xffu;
    let out_g_raw = (lut[g] >> 8u) & 0xffu;
    let out_b_raw = (lut[b] >> 16u) & 0xffu;

    let out_g = select(g, out_g_raw, mode_has_g(params.mode));
    let out_b = select(b, out_b_raw, mode_has_b(params.mode));
    // Alpha preserved unchanged

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (a << 24u);
}

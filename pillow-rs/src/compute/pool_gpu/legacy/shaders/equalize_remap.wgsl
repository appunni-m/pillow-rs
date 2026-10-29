// Equalize remap: apply histogram-equalization LUT to each channel.
// Per-pixel dispatch (16x16 workgroups).
//
// CPU reference: out[c] = lut[in[c]]
// Reads the 256-entry LUT from storage buffer at binding(3).
// Same pattern as autocontrast_remap but reads a pre-computed LUT
// instead of computing a linear stretch.
//
// Mode-aware: L/LA only remap R channel; RGB remap R,G,B; RGBA remap R,G,B,A.
// Params: standard header.

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
@group(0) @binding(3) var<storage, read> lut: array<u32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let out_r = lut[r];
    let out_g = select(g, lut[g], mode_has_g(params.mode));
    let out_b = select(b, lut[b], mode_has_b(params.mode));
    // Alpha preserved unchanged

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (a << 24u);
}

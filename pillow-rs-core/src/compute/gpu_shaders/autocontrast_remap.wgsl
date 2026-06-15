// Autocontrast remap: linear stretch of pixel values using scale/offset.
// Per-pixel dispatch (16x16 workgroups).
//
// CPU reference: lut[in] = clamp((in * scale + offset).round(), 0, 255)
// For each channel: out[c] = clamp((c * scale + offset + 0.5) as i32, 0, 255)
//
// Mode-aware: L/LA only remap R channel; RGB remap R,G,B; RGBA remap R,G,B,A.
// Params: scale_bits (f32), offset_bits (f32) after standard header.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    scale_bits: u32,
    offset_bits: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn remap_channel(c: u32, scale: f32, offset: f32) -> u32 {
    let val = f32(c) * scale + offset + 0.5;
    let clamped = clamp(i32(val), 0, 255);
    return u32(clamped);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let scale = bitcast<f32>(params.scale_bits);
    let offset = bitcast<f32>(params.offset_bits);

    let out_r = remap_channel(r, scale, offset);
    let out_g = select(g, remap_channel(g, scale, offset), mode_has_g(params.mode));
    let out_b = select(b, remap_channel(b, scale, offset), mode_has_b(params.mode));
    // Alpha preserved unchanged

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (a << 24u);
}

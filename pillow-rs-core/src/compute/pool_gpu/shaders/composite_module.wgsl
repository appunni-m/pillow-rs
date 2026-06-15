// CompositeModule: 3-input blend with mask
// out = (a * (255 - mask) + b * mask) / 255
// Alpha preserved from input_a
//
// Mode-aware: only blend active channels.
// L/LA: blend R only; RGB: blend R,G,B; RGBA: blend R,G,B (alpha preserved from a).
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

@group(0) @binding(0) var<storage, read> input_a: array<u32>;
@group(0) @binding(1) var<storage, read> input_b: array<u32>;
@group(0) @binding(2) var<storage, read> mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;

    let pa = input_a[idx];
    let pb = input_b[idx];
    let pm = mask[idx];
    let ar = pa & 0xffu;
    let ag = (pa >> 8u) & 0xffu;
    let ab = (pa >> 16u) & 0xffu;
    let aa = (pa >> 24u) & 0xffu;
    let br = pb & 0xffu;
    let bg = (pb >> 8u) & 0xffu;
    let bb = (pb >> 16u) & 0xffu;
    let mr = pm & 0xffu;

    let mode = params.mode;

    let out_r = (ar * (255u - mr) + br * mr) / 255u;
    let out_g = select(ag, (ag * (255u - mr) + bg * mr) / 255u, mode_has_g(mode));
    let out_b = select(ab, (ab * (255u - mr) + bb * mr) / 255u, mode_has_b(mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (aa << 24u);
}

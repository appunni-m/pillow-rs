// Composite: ImageChops.composite(image1, image2, mask)
// 3-input blend with mask.
//   input_a = image1 (img)
//   input_b = image2 (other)
//   mask    = blend factor
// Formula: out = a * mask + b * (255 - mask)
// Alpha preserved from input_a.
// Same algorithm as composite_module.wgsl. Separate shader for clarity.
//
// 5-binding layout (buf_img3): input_a, input_b, mask, output, params.
//   Same layout as composite_module.wgsl.
//
// Mode-aware: only blend active channels.
// L/LA: blend R only; RGB: blend R,G,B; RGBA: blend R,G,B (alpha preserved from a).
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

@group(0) @binding(0) var<storage, read> input_a: array<u32>;
@group(0) @binding(1) var<storage, read> input_b: array<u32>;
@group(0) @binding(2) var<storage, read> input_mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;

    let pa = input_a[idx];
    let pb = input_b[idx];
    let pm = input_mask[idx];

    let ar = pa & 0xffu;
    let ag = (pa >> 8u) & 0xffu;
    let ab = (pa >> 16u) & 0xffu;
    let aa = (pa >> 24u) & 0xffu;

    let br = pb & 0xffu;
    let bg = (pb >> 8u) & 0xffu;
    let bb = (pb >> 16u) & 0xffu;

    let mr = pm & 0xffu;  // mask in low byte
    let inv_mask = 255u - mr;

    // out = a * mask + b * (255 - mask)
    let mode = params.mode;
    let out_r = (ar * mr + br * inv_mask) / 255u;
    let out_g_raw = (ag * mr + bg * inv_mask) / 255u;
    let out_b_raw = (ab * mr + bb * inv_mask) / 255u;

    let out_g = select(ag, out_g_raw, mode_has_g(mode));
    let out_b = select(ab, out_b_raw, mode_has_b(mode));
    // Alpha preserved from input_a

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (aa << 24u);
}

// Color saturation: luma-preserving blend
// luma = BT.601, then lerp(luma, ch, factor)

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
    factor_int: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn lerp_fn(ch: u32, luma: u32, f: u32) -> u32 {
    let inv_f = 1000u - f;
    return min((luma * inv_f + ch * f) / 1000u, 255u);
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

    // BT.601 luma
    let luma = (299u * r + 587u * g + 114u * b) / 1000u;

    // lerp: luma * (1000 - f) + ch * f over 1000
    let f = params.factor_int;

    output[idx] = lerp_fn(r, luma, f) | (lerp_fn(g, luma, f) << 8u) | (lerp_fn(b, luma, f) << 16u) | (a << 24u);
}

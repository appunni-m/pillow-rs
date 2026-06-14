// Brightness: clamp(ch * factor_int / 1000, 0, 255)
// Param[0] = factor * 1000

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

fn brightness_apply(c: u32, f: u32) -> u32 {
    return min((c * f) / 1000u, 255u);
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

    let f = params.factor_int;

    output[idx] = brightness_apply(r, f) | (brightness_apply(g, f) << 8u) | (brightness_apply(b, f) << 16u) | (a << 24u);
}

// Contrast: clamp((ch - 128) * factor_int / 1000 + 128, 0, 255)
// Uses signed i32 arithmetic for negative factors

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

fn contrast_apply(c: u32, fi: i32) -> u32 {
    let ci = i32(c);
    let val = ((ci - 128) * fi) / 1000 + 128;
    return u32(clamp(val, 0, 255));
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

    let fi = i32(params.factor_int);

    output[idx] = contrast_apply(r, fi) | (contrast_apply(g, fi) << 8u) | (contrast_apply(b, fi) << 16u) | (a << 24u);
}

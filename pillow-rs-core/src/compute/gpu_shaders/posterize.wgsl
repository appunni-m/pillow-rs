// Posterize: quantize using (c * levels + 128) / 256 * 256 / levels
// Param[0] = bits

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
    bits: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn posterize_fn(c: u32, levels: u32) -> u32 {
    let q = (c * levels + 128u) / 256u * 256u / levels;
    return min(q, 255u);
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

    let bits = params.bits;
    let levels = 1u << bits;
    // PIL: ((c * levels + 128) / 256) * 256 / levels, clamped to 0-255

    output[idx] = posterize_fn(r, levels) | (posterize_fn(g, levels) << 8u) | (posterize_fn(b, levels) << 16u) | (a << 24u);
}

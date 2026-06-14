// Solarize: if ch > threshold, ch = 255 - ch
// Param[0] = threshold

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
    threshold: u32,
}

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

    let thresh = params.threshold;
    let out_r = select(r, 255u - r, r > thresh);
    let out_g = select(g, 255u - g, g > thresh);
    let out_b = select(b, 255u - b, b > thresh);

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (a << 24u);
}

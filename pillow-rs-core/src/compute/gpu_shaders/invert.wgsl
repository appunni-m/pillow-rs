// Invert: out = 255 - in per channel
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) wgs: vec3<u32>) {
    let width = wgs.x * 16u;
    let total = width * (wgs.y * 16u);
    let idx = gid.y * width + gid.x;
    if idx >= total { return; }

    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let out_r = 255u - r;
    let out_g = 255u - g;
    let out_b = 255u - b;

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (a << 24u);
}

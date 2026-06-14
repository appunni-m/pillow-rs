// Grayscale: BT.601 luma with integer arithmetic
// luma = (299*r + 587*g + 114*b + 500) / 1000

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

    let luma = (299u * r + 587u * g + 114u * b + 500u) / 1000u;
    let luma_clamped = min(luma, 255u);

    // Output as RGB (all channels = luma) for RGB mode preservation
    output[idx] = luma_clamped | (luma_clamped << 8u) | (luma_clamped << 16u) | (a << 24u);
}

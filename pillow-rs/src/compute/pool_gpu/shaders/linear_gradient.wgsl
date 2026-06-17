// LinearGradient: vertical ramp from 0 to 255 over height.
// Research §2: Per-pixel compute, trivially parallel — each pixel calculates
// its own value from (y / height) independent of neighbors.

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read_write> output: array<u32>;

@group(0) @binding(1) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let value = u32(f32(gid.y) * 255.0 / f32(params.height - 1u));
    output[idx] = value | (value << 8u) | (value << 16u) | 0xff000000u;
}

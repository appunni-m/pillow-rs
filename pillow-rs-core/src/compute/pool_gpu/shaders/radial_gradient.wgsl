// RadialGradient: radial ramp from center, 0 at center → 255 at corners.
// Research §2: Per-pixel distance from center, trivially parallel.

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
    let cx = f32(params.width) * 0.5;
    let cy = f32(params.height) * 0.5;
    let dx = f32(gid.x) - cx;
    let dy = f32(gid.y) - cy;
    let max_dist = sqrt(cx * cx + cy * cy);
    let dist = sqrt(dx * dx + dy * dy);
    let value = u32(clamp(dist / max_dist, 0.0, 1.0) * 255.0);
    output[idx] = value | (value << 8u) | (value << 16u) | 0xff000000u;
}

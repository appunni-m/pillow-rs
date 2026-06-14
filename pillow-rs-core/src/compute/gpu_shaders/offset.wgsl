// Offset: output[y][x] = input[clamp(y-dy,0,H-1)][clamp(x-dx,0,W-1)]
// Param[0] = dx, Param[1] = dy

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
    dx: u32,
    dy: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let w = params.width;
    let h = params.height;
    let dx = params.dx;
    let dy = params.dy;

    // Source: read from (x-dx, y-dy) with wrapping
    let sx = (gid.x + w - dx) % w;
    let sy = (gid.y + h - dy) % h;
    let src_idx = sy * w + sx;
    let dst_idx = gid.y * w + gid.x;

    output[dst_idx] = input[src_idx];
}

// ExtractBand: extract single channel from multi-band image.
// Research §2: Simple per-pixel copy, trivially parallel.
// channel: 0=R/luma, 1=G/A, 2=B, 3=A

struct Params {
    width: u32,
    height: u32,
    channel: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let shift = params.channel * 8u;
    let value = (pixel >> shift) & 0xffu;
    // Output as Luma (stored in R channel, compatible with L8 format)
    output[idx] = value | (value << 8u) | (value << 16u) | 0xff000000u;
}

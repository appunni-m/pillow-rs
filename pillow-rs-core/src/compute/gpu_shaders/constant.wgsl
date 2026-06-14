// Constant: all output pixels = packed u32 value
// Param[0] = packed color value 0xAABBGGRR

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
    value: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    output[idx] = params.value;
}

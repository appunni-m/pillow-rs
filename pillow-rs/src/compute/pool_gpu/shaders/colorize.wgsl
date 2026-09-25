// Colorize: use the exact, host-built Pillow LUT for each grayscale sample.
// The packed result is RGB in the low three bytes and opaque alpha in byte 3.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> lut: array<u32, 256>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let index = gid.y * params.width + gid.x;
    let luma = input[index] & 0xffu;
    output[index] = lut[luma];
}

// Add one alpha sample to each packed three-byte RGB pixel. The source buffer
// keeps its native RGB byte layout; the output buffer uses the required RGBA
// layout, with each invocation owning exactly one destination pixel.

struct Params {
    width: u32,
    height: u32,
    alpha: u32,
    input_bytes: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn read_byte(byte_index: u32) -> u32 {
    let word = input[byte_index >> 2u];
    let shift = (byte_index & 3u) * 8u;
    return (word >> shift) & 0xffu;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }

    let pixel = gid.y * params.width + gid.x;
    let source = pixel * 3u;
    if source + 2u >= params.input_bytes {
        return;
    }
    let red = read_byte(source);
    let green = read_byte(source + 1u);
    let blue = read_byte(source + 2u);
    output[pixel] = red | (green << 8u) | (blue << 16u) | ((params.alpha & 0xffu) << 24u);
}

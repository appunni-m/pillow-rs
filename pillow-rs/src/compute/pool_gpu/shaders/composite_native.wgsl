// Native-byte ImageChops.composite for matching images and a same-size mask.
// Each invocation packs four result bytes into one storage word so L/LA/RGB
// and RGBA avoid expansion to a four-byte-per-pixel transport representation.

struct Params {
    channels: u32,
    mask_channels: u32,
    mask_channel: u32,
    byte_length: u32,
    word_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> input_a: array<u32>;
@group(0) @binding(1) var<storage, read> input_b: array<u32>;
@group(0) @binding(2) var<storage, read> input_mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

fn read_a(byte_index: u32) -> u32 {
    let word = input_a[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_b(byte_index: u32) -> u32 {
    let word = input_b[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_mask(byte_index: u32) -> u32 {
    let word = input_mask[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let word_index = gid.x;
    if word_index >= params.word_count { return; }

    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let byte_index = word_index * 4u + lane;
        if byte_index < params.byte_length {
            let pixel_index = byte_index / params.channels;
            let mask_index = pixel_index * params.mask_channels + params.mask_channel;
            let source = read_a(byte_index);
            let destination = read_b(byte_index);
            let mask = read_mask(mask_index);
            let inverse_mask = 255u - mask;
            let value = (source * mask + destination * inverse_mask + 127u) / 255u;
            packed |= value << (lane * 8u);
        }
    }
    output[word_index] = packed;
}

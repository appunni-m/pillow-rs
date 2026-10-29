// Equalize remap for native packed L storage.
// Each invocation owns one complete u32 containing four L samples.

struct Params {
    width: u32,
    height: u32,
    _mode: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> lut: array<u32, 256>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let total_pixels = params.width * params.height;
    let word_count = (total_pixels + 3u) / 4u;
    let word_index = gid.x;
    if word_index >= word_count {
        return;
    }

    let packed = input[word_index];
    let value0 = packed & 0xffu;
    let value1 = (packed >> 8u) & 0xffu;
    let value2 = (packed >> 16u) & 0xffu;
    let value3 = (packed >> 24u) & 0xffu;
    let mapped0 = lut[value0] & 0xffu;
    let mapped1 = lut[value1] & 0xffu;
    let mapped2 = lut[value2] & 0xffu;
    let mapped3 = lut[value3] & 0xffu;

    output[word_index] = mapped0 | (mapped1 << 8u) | (mapped2 << 16u) | (mapped3 << 24u);
}

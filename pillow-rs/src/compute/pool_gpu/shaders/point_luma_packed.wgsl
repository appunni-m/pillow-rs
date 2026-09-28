// Point for native L images. Four adjacent one-byte samples share a storage
// word so the host can upload and read back one byte per pixel instead of
// widening every sample to RGBA.

struct Params {
    word_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> lut: array<u32, 256>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let word_index = gid.x;
    if word_index >= params.word_count {
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

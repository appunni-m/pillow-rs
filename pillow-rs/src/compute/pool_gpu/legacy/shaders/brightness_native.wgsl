// Native byte transport for brightness. Each invocation scales the color
// samples in one packed word; LA alpha is left untouched.

struct Params {
    columns: u32,
    rows: u32,
    channels: u32,
    words: u32,
    factor_int: u32,
    byte_len: u32,
    active_channels: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.columns || gid.y >= params.rows { return; }

    let word_index = gid.y * params.columns + gid.x;
    if word_index >= params.words { return; }

    let source = input[word_index];
    var result = source;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let sample_index = word_index * 4u + lane;
        if sample_index < params.byte_len
            && (params.active_channels == params.channels
                || sample_index % params.channels < params.active_channels) {
            let shift = lane * 8u;
            let sample = (source >> shift) & 0xffu;
            let scaled = min((sample * params.factor_int) / 1000u, 255u);
            let mask = 0xffu << shift;
            result = (result & ~mask) | (scaled << shift);
        }
    }
    output[word_index] = result;
}
